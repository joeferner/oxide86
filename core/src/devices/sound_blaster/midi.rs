use std::{
    io::Cursor,
    sync::{Arc, OnceLock},
};

use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

use crate::devices::PcmRingBuffer;

static SF2_BYTES: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/gm.sf2"));

static SOUND_FONT: OnceLock<Arc<SoundFont>> = OnceLock::new();

fn shared_sound_font() -> Arc<SoundFont> {
    SOUND_FONT
        .get_or_init(|| {
            let mut cursor = Cursor::new(SF2_BYTES);
            Arc::new(SoundFont::new(&mut cursor).expect("bundled GM soundfont invalid"))
        })
        .clone()
}

pub(super) struct SoundBlasterMidi {
    synth: Synthesizer,
    out: PcmRingBuffer,
    cpu_freq: u64,
    last_cycle: u32,
    sample_acc: u64,
    parser: MidiParser,
}

impl SoundBlasterMidi {
    pub(super) fn new(cpu_freq: u64) -> Self {
        let sf = shared_sound_font();
        let settings = SynthesizerSettings::new(44100);
        let synth = Synthesizer::new(&sf, &settings).expect("MIDI synthesizer init failed");
        Self {
            synth,
            out: PcmRingBuffer::new(44100 * 2, 44100),
            cpu_freq,
            last_cycle: 0,
            sample_acc: 0,
            parser: MidiParser::new(),
        }
    }

    pub(super) fn consumer(&self) -> PcmRingBuffer {
        self.out.clone()
    }

    pub(super) fn push_byte(&mut self, byte: u8) {
        if let Some((status, d0, d1)) = self.parser.push_byte(byte) {
            let channel = (status & 0x0F) as i32;
            let command = (status & 0xF0) as i32;
            self.synth
                .process_midi_message(channel, command, d0 as i32, d1 as i32);
        }
    }

    pub(super) fn advance_to_cycle(&mut self, cycle_count: u32) {
        let elapsed = cycle_count.wrapping_sub(self.last_cycle) as u64;
        self.last_cycle = cycle_count;

        self.sample_acc += elapsed * 44100;
        let n = (self.sample_acc / self.cpu_freq) as usize;
        self.sample_acc %= self.cpu_freq;

        if n == 0 {
            return;
        }

        let mut left = vec![0.0f32; n];
        let mut right = vec![0.0f32; n];
        self.synth.render(&mut left, &mut right);
        for (l, r) in left.iter().zip(right.iter()) {
            self.out.push_sample((l + r) * 0.5);
        }
    }

    pub(super) fn reset(&mut self) {
        self.synth.reset();
        self.out.clear();
        self.parser = MidiParser::new();
        self.sample_acc = 0;
    }
}

struct MidiParser {
    status: u8,
    data: [u8; 2],
    data_pos: usize,
    in_sysex: bool,
}

impl MidiParser {
    fn new() -> Self {
        Self {
            status: 0,
            data: [0; 2],
            data_pos: 0,
            in_sysex: false,
        }
    }

    /// Feed one MIDI byte. Returns `Some((status, data0, data1))` when a
    /// complete channel message has been assembled; `None` otherwise.
    fn push_byte(&mut self, byte: u8) -> Option<(u8, u8, u8)> {
        // Realtime messages (0xF8–0xFF): single byte, do not disturb running status.
        if byte >= 0xF8 {
            return None;
        }
        // Inside a SysEx: swallow bytes until the terminator (0xF7).
        if self.in_sysex {
            if byte == 0xF7 {
                self.in_sysex = false;
                self.status = 0;
            }
            return None;
        }
        // Status bytes.
        if byte >= 0x80 {
            if byte == 0xF0 {
                self.in_sysex = true;
                self.status = 0;
                return None;
            }
            if byte >= 0xF1 {
                // System-common messages clear running status.
                self.status = 0;
                return None;
            }
            self.status = byte;
            self.data_pos = 0;
            return None;
        }
        // Data byte — requires a current running status.
        if self.status == 0 {
            return None;
        }
        self.data[self.data_pos] = byte;
        self.data_pos += 1;
        if self.data_pos >= self.data_len() {
            self.data_pos = 0;
            Some((self.status, self.data[0], self.data[1]))
        } else {
            None
        }
    }

    fn data_len(&self) -> usize {
        match self.status & 0xF0 {
            0x80 | 0x90 | 0xA0 | 0xB0 | 0xE0 => 2,
            0xC0 | 0xD0 => 1,
            _ => 0,
        }
    }
}
