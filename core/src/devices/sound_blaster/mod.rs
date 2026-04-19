use std::{any::Any, collections::VecDeque};

use crate::{
    Device,
    devices::{CdromController, PcmRingBuffer, SoundCard},
    disk::cdrom::CdromBackend,
    utils::bcd_to_dec,
};

mod dsp;
use dsp::SoundBlasterDsp;
mod mixer;
use mixer::SoundBlasterMixer;
mod midi;
use midi::SoundBlasterMidi;
mod mpu;
use mpu::SoundBlasterMpu;
mod opl;
use opl::SoundBlasterOpl;

// Status byte bits (read from base+0)
const STATUS_RESULT_AVAIL: u8 = 0x01;
const STATUS_BUSY: u8 = 0x02;
const STATUS_ERROR: u8 = 0x04;
const STATUS_DISC_PRESENT: u8 = 0x20;
const STATUS_DOOR_OPEN: u8 = 0x40;

const RESULT_OK: u8 = 0x00;
const RESULT_ERROR: u8 = 0x05;

/// Converts MSF (minutes, seconds, frames) to LBA.
/// LBA = (M * 60 + S) * 75 + F - 150
fn msf_to_lba(m: u8, s: u8, f: u8) -> u32 {
    let lba = (m as u32 * 60 + s as u32) * 75 + f as u32;
    lba.saturating_sub(150)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CdromState {
    Idle,
    RecvParams(u8),
    Execute,
    SendResult,
    StreamSector,
}

/// Panasonic/Matsushita CD interface state, absorbed from `SoundBlasterCdrom`.
///
/// IO port layout (base configurable, default 0x230):
/// - base+0  R: result data byte (SendResult) / sector data (StreamSector) / status (otherwise)   W: command/param
/// - base+1  R: busy flag / sector data (StreamSector after result consumed)   W: param byte
/// - base+2  R: extended status  W: reset (0xFF)
/// - base+3  R: drive select read-back  W: drive select (bits 0–1)
struct SoundBlasterCdromInner {
    base_port: u16,
    drive_selected: u8,
    disc: Option<Box<dyn CdromBackend>>,
    state: CdromState,
    command: u8,
    params: Vec<u8>,
    result_buf: VecDeque<u8>,
    read_lba: u32,
    read_remaining: u32,
    read_sector_buf: [u8; 2048],
    read_sector_pos: usize,
    door_open: bool,
    error: bool,
    pending_irq: bool,
    irq_enabled: bool,
    irq_line: u8,
    attention_pending: bool,
    stream_via_base0: bool,
}

impl SoundBlasterCdromInner {
    fn new(base_port: u16, disc: Option<Box<dyn CdromBackend>>, irq_line: u8) -> Self {
        let door_open = false;
        let attention_pending = disc.is_some();
        Self {
            base_port,
            drive_selected: 0,
            disc,
            state: CdromState::Idle,
            command: 0,
            params: Vec::new(),
            result_buf: VecDeque::new(),
            read_lba: 0,
            read_remaining: 0,
            read_sector_buf: [0u8; 2048],
            read_sector_pos: 0,
            door_open,
            error: false,
            pending_irq: false,
            irq_enabled: true,
            irq_line,
            attention_pending,
            stream_via_base0: false,
        }
    }

    fn disc_present(&self) -> bool {
        self.disc.is_some()
    }

    fn status_byte(&self) -> u8 {
        let mut s = 0u8;
        if matches!(
            self.state,
            CdromState::SendResult | CdromState::StreamSector
        ) {
            s |= STATUS_RESULT_AVAIL;
        }
        if matches!(self.state, CdromState::RecvParams(_) | CdromState::Execute) {
            s |= STATUS_BUSY;
        }
        if self.error {
            s |= STATUS_ERROR;
        }
        if self.disc_present() {
            s |= STATUS_DISC_PRESENT;
        }
        if self.door_open {
            s |= STATUS_DOOR_OPEN;
        }
        s
    }

    fn params_for_command(cmd: u8) -> u8 {
        match cmd {
            0x00 => 0,
            0x01 => 0,
            0x02 => 6,
            0x05 => 0,
            0x09 => 1,
            0x0A => 3,
            0x0B => 7,
            0x0C => 0,
            0x0D => 0,
            0x10 => 0,
            0x11 => 2,
            0x12 => 0,
            0x81 => 0,
            0x83 => 0,
            0x84 => 6,
            0x88 => 0,
            0x8B => 0,
            _ => 0,
        }
    }

    fn execute_command(&mut self) {
        self.error = false;
        self.result_buf.clear();

        match self.command {
            0x83 => {
                for &b in b"MATSHITA" {
                    self.result_buf.push_back(b);
                }
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.state = CdromState::SendResult;
            }

            0x00 => {
                self.result_buf.push_back(0xAA);
                self.result_buf.push_back(0x55);
                self.state = CdromState::SendResult;
            }

            0x01 => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            0x81 => {
                let result = if self.disc_present() {
                    0x08 | 0x40 | 0x01
                } else {
                    0x08
                };
                self.result_buf.push_back(result);
                self.state = CdromState::SendResult;
            }

            0x05 => {
                self.result_buf.push_back(RESULT_OK);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.state = CdromState::SendResult;
            }

            0x09 => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            0x0A => {
                if !self.disc_present() {
                    self.error = true;
                    self.result_buf.push_back(RESULT_ERROR);
                    self.state = CdromState::SendResult;
                    if self.irq_enabled {
                        self.pending_irq = true;
                    }
                    return;
                }
                self.read_lba = msf_to_lba(self.params[0], self.params[1], self.params[2]);
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            0x0B => {
                if !self.disc_present() {
                    self.error = true;
                    self.result_buf.push_back(RESULT_ERROR);
                    self.state = CdromState::SendResult;
                    if self.irq_enabled {
                        self.pending_irq = true;
                    }
                    return;
                }
                let lba = msf_to_lba(self.params[0], self.params[1], self.params[2]);
                let count = (self.params[3] as u32) << 16
                    | (self.params[4] as u32) << 8
                    | (self.params[5] as u32);
                self.read_lba = lba;
                self.read_remaining = count;
                self.read_sector_pos = 2048;
                self.result_buf.push_back(RESULT_OK);
                if count > 0 {
                    self.state = CdromState::StreamSector;
                } else {
                    self.state = CdromState::SendResult;
                }
            }

            0x0C => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            0x0D => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            0x10 => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            0x11 => {
                if !self.disc_present() {
                    self.error = true;
                    self.result_buf.push_back(RESULT_ERROR);
                    self.state = CdromState::SendResult;
                    if self.irq_enabled {
                        self.pending_irq = true;
                    }
                    return;
                }
                let first = self.params[0];
                let last = self.params[1];
                self.result_buf.push_back(RESULT_OK);
                self.result_buf.push_back(first);
                self.result_buf.push_back(last);
                for _track in first..=last {
                    self.result_buf.push_back(0x00);
                    self.result_buf.push_back(0x00);
                    self.result_buf.push_back(0x02);
                    self.result_buf.push_back(0x00);
                }
                self.state = CdromState::SendResult;
            }

            0x12 => {
                if !self.disc_present() {
                    self.error = true;
                    self.result_buf.push_back(RESULT_ERROR);
                    self.state = CdromState::SendResult;
                    if self.irq_enabled {
                        self.pending_irq = true;
                    }
                    return;
                }
                let total = self.disc.as_ref().map(|d| d.total_sectors()).unwrap_or(0);
                let total_frames = total + 150;
                let f = (total_frames % 75) as u8;
                let total_seconds = total_frames / 75;
                let s = (total_seconds % 60) as u8;
                let m = (total_seconds / 60) as u8;
                self.result_buf.push_back(RESULT_OK);
                self.result_buf.push_back(0x01);
                self.result_buf.push_back(0x01);
                self.result_buf.push_back(m);
                self.result_buf.push_back(s);
                self.result_buf.push_back(f);
                self.state = CdromState::SendResult;
            }

            0x88 => {
                let total = self.disc.as_ref().map(|d| d.total_sectors()).unwrap_or(0);
                let total_frames = total + 150;
                let f = (total_frames % 75) as u8;
                let total_seconds = total_frames / 75;
                let s = (total_seconds % 60) as u8;
                let m = (total_seconds / 60) as u8;
                self.result_buf.extend(&[m, s, f, 0x08, 0x00]);
                self.state = CdromState::SendResult;
            }

            0x8B => {
                self.result_buf.extend(&[0x00u8; 6]);
                self.state = CdromState::SendResult;
            }

            0x02 => {
                if !self.disc_present() {
                    self.error = true;
                    self.result_buf.push_back(RESULT_ERROR);
                    self.state = CdromState::SendResult;
                    if self.irq_enabled {
                        self.pending_irq = true;
                    }
                    return;
                }
                let m = bcd_to_dec(self.params[0]);
                let s = bcd_to_dec(self.params[1]);
                let f = bcd_to_dec(self.params[2]);
                let count = (self.params[3] as u32) << 8 | self.params[4] as u32;
                self.read_lba = msf_to_lba(m, s, f);
                self.read_remaining = count;
                self.read_sector_pos = 2048;
                self.stream_via_base0 = true;
                if count > 0 {
                    self.state = CdromState::StreamSector;
                } else {
                    self.state = CdromState::Idle;
                }
            }

            0x84 => {
                self.state = CdromState::Idle;
            }

            0x0E | 0x0F => {
                log::warn!(
                    "SoundBlaster: unimplemented audio command {:#04x}",
                    self.command
                );
                self.error = true;
                self.result_buf.push_back(RESULT_ERROR);
                self.state = CdromState::SendResult;
            }

            _ => {
                log::warn!("SoundBlaster: unknown CD-ROM command {:#04x}", self.command);
                self.error = true;
                self.result_buf.push_back(RESULT_ERROR);
                self.state = CdromState::SendResult;
            }
        }

        let irq_on_stream =
            matches!(self.state, CdromState::StreamSector) && !self.stream_via_base0;
        if (matches!(self.state, CdromState::SendResult) || irq_on_stream) && self.irq_enabled {
            self.pending_irq = true;
        }
    }

    fn stream_read_byte(&mut self) -> u8 {
        if self.read_sector_pos >= 2048 {
            if self.read_remaining == 0 {
                self.state = CdromState::Idle;
                return 0x00;
            }
            match self.disc.as_mut() {
                Some(disc) => {
                    let mut buf = [0u8; 2048];
                    if disc.read_sector(self.read_lba, &mut buf).is_ok() {
                        self.read_sector_buf = buf;
                    } else {
                        log::warn!("SoundBlaster: read_sector failed at LBA {}", self.read_lba);
                        self.error = true;
                        self.state = CdromState::Idle;
                        return RESULT_ERROR;
                    }
                }
                None => {
                    self.error = true;
                    self.state = CdromState::Idle;
                    return RESULT_ERROR;
                }
            }
            self.read_lba += 1;
            self.read_remaining -= 1;
            self.read_sector_pos = 0;
            if self.read_remaining == 0 && !self.stream_via_base0 {
                self.state = CdromState::Idle;
            }
        }

        let byte = self.read_sector_buf[self.read_sector_pos];
        self.read_sector_pos += 1;
        byte
    }

    fn handle_write_base0(&mut self, val: u8) {
        match self.state {
            CdromState::Idle => {
                self.command = val;
                let n = Self::params_for_command(val);
                self.params.clear();
                if n == 0 {
                    self.state = CdromState::Execute;
                    self.execute_command();
                } else {
                    self.state = CdromState::RecvParams(n);
                }
            }
            CdromState::RecvParams(remaining) => {
                self.params.push(val);
                let new_remaining = remaining - 1;
                if new_remaining == 0 {
                    self.state = CdromState::Execute;
                    self.execute_command();
                } else {
                    self.state = CdromState::RecvParams(new_remaining);
                }
            }
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.state = CdromState::Idle;
        self.command = 0;
        self.params.clear();
        self.result_buf.clear();
        self.read_remaining = 0;
        self.read_sector_pos = 2048;
        self.error = false;
        self.pending_irq = false;
        self.attention_pending = false;
        self.stream_via_base0 = false;
    }

    fn io_read_u8(&mut self, port: u16, _cycle_count: u32) -> Option<u8> {
        let offset = port.wrapping_sub(self.base_port);
        match offset {
            0 => {
                let byte = match self.state {
                    CdromState::SendResult => {
                        let b = self.result_buf.pop_front().unwrap_or(0x00);
                        if self.result_buf.is_empty() {
                            self.state = CdromState::Idle;
                        }
                        b
                    }
                    CdromState::StreamSector if !self.result_buf.is_empty() => {
                        self.result_buf.pop_front().unwrap()
                    }
                    CdromState::StreamSector if self.stream_via_base0 => self.stream_read_byte(),
                    _ => self.status_byte(),
                };
                log::debug!("SbCd R base+0 val=0x{byte:02X} state={:?}", self.state);
                Some(byte)
            }
            1 => {
                let byte = match self.state {
                    CdromState::RecvParams(_) | CdromState::Execute => 0x04,
                    CdromState::StreamSector
                        if self.result_buf.is_empty() && !self.stream_via_base0 =>
                    {
                        self.stream_read_byte()
                    }
                    CdromState::StreamSector if self.stream_via_base0 => {
                        if self.read_sector_pos >= 2048 && self.read_remaining == 0 {
                            self.state = CdromState::Idle;
                            self.stream_via_base0 = false;
                            0x01
                        } else {
                            0x00
                        }
                    }
                    CdromState::Idle if self.attention_pending => {
                        self.attention_pending = false;
                        0x01
                    }
                    _ => 0x00,
                };
                log::debug!("SbCd R base+1 val=0x{byte:02X} state={:?}", self.state);
                Some(byte)
            }
            2 => {
                let s = self.status_byte();
                log::debug!("SbCd R base+2 ext_status=0x{s:02X}");
                Some(s)
            }
            3 => {
                log::debug!("SbCd R base+3 drive_selected={}", self.drive_selected);
                Some(self.drive_selected)
            }
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        let offset = port.wrapping_sub(self.base_port);
        match offset {
            0 => {
                log::debug!("SbCd W base+0 val=0x{val:02X} state={:?}", self.state);
                self.handle_write_base0(val);
                true
            }
            1 => {
                log::debug!("SbCd W base+1 val=0x{val:02X} state={:?}", self.state);
                if let CdromState::RecvParams(remaining) = self.state {
                    self.params.push(val);
                    let new_remaining = remaining - 1;
                    if new_remaining == 0 {
                        self.state = CdromState::Execute;
                        self.execute_command();
                    } else {
                        self.state = CdromState::RecvParams(new_remaining);
                    }
                }
                true
            }
            2 => {
                log::debug!("SbCd W base+2 val=0x{val:02X}");
                if val == 0xFF {
                    self.reset();
                }
                true
            }
            3 => {
                log::debug!("SbCd W base+3 drive_select=0x{val:02X}");
                self.drive_selected = val & 0x03;
                true
            }
            _ => false,
        }
    }

    fn load_disc(&mut self, disc: Box<dyn CdromBackend>) {
        self.disc = Some(disc);
        self.door_open = false;
        self.error = false;
        self.state = CdromState::Idle;
        self.result_buf.clear();
        self.attention_pending = true;
    }

    fn eject_disc(&mut self) {
        self.disc = None;
        self.door_open = true;
        self.state = CdromState::Idle;
        self.result_buf.clear();
        self.read_remaining = 0;
        self.attention_pending = false;
    }

    fn take_pending_irq(&mut self) -> bool {
        if self.pending_irq {
            self.pending_irq = false;
            true
        } else {
            false
        }
    }

    fn irq_line(&self) -> u8 {
        self.irq_line
    }
}

/// Sound Blaster 16 emulation.
///
/// Phase 2: wraps the Panasonic CD-ROM interface (absorbed from `SoundBlasterCdrom`).
/// Phase 4: adds DSP subsystem — reset handshake and basic commands.
/// Phase 7: adds 8-bit DMA PCM playback via `pcm_out` ring buffer.
/// Phase 8: adds MPU-401 MIDI controller in UART mode (ports 0x330/0x331).
/// Phase 9: adds MIDI synthesizer (rustysynth + bundled GM SoundFont).
pub struct SoundBlaster {
    base_port: u16,
    cdrom: SoundBlasterCdromInner,
    dsp: SoundBlasterDsp,
    midi: SoundBlasterMidi,
    mixer: SoundBlasterMixer,
    mpu: SoundBlasterMpu,
    opl: SoundBlasterOpl,
    pcm_out: crate::devices::PcmRingBuffer,
    cpu_freq: u64,
    /// Last Direct DAC sample value (pending, not yet pushed to pcm_out).
    last_dac_sample: f32,
    /// Cycle count when the last Direct DAC sample arrived.
    last_dac_cycle: Option<u32>,
}

impl SoundBlaster {
    /// Create with default SB base 0x220, CD-ROM port 0x230, no disc, IRQ 5. Used by tests.
    pub fn new(cpu_freq: u64) -> Self {
        Self {
            base_port: 0x220,
            cdrom: SoundBlasterCdromInner::new(0x230, None, 5),
            dsp: SoundBlasterDsp::new(),
            midi: SoundBlasterMidi::new(cpu_freq),
            mixer: SoundBlasterMixer::new(),
            mpu: SoundBlasterMpu::new(),
            opl: SoundBlasterOpl::new(cpu_freq),
            pcm_out: PcmRingBuffer::new_with_hold(44100 * 2, 44100),
            cpu_freq,
            last_dac_sample: 0.0,
            last_dac_cycle: None,
        }
    }

    /// Create with explicit CD-ROM configuration for native/CLI use.
    pub fn with_cdrom(
        cdrom_base_port: u16,
        disc: Option<Box<dyn CdromBackend>>,
        irq_line: u8,
        cpu_freq: u64,
    ) -> Self {
        Self {
            base_port: 0x220,
            cdrom: SoundBlasterCdromInner::new(cdrom_base_port, disc, irq_line),
            dsp: SoundBlasterDsp::new(),
            midi: SoundBlasterMidi::new(cpu_freq),
            mixer: SoundBlasterMixer::new(),
            mpu: SoundBlasterMpu::new(),
            opl: SoundBlasterOpl::new(cpu_freq),
            pcm_out: PcmRingBuffer::new_with_hold(44100 * 2, 44100),
            cpu_freq,
            last_dac_sample: 0.0,
            last_dac_cycle: None,
        }
    }

    pub fn midi_consumer(&self) -> PcmRingBuffer {
        self.midi.consumer()
    }

    pub fn opl_consumer(&self) -> PcmRingBuffer {
        self.opl.consumer()
    }

    pub fn pcm_consumer(&self) -> PcmRingBuffer {
        self.pcm_out.clone()
    }
}

impl Device for SoundBlaster {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.cdrom.reset();
        self.dsp.hardware_reset();
        self.midi.reset();
        self.mixer.reset();
        self.mpu.reset();
        self.opl.reset();
        self.pcm_out.clear();
        self.last_dac_sample = 0.0;
        self.last_dac_cycle = None;
    }

    fn memory_read_u8(&mut self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn dma_write_u8(&mut self, val: u8) -> bool {
        let (sample, _done) = self.dsp.dma_receive_byte(val);
        // Upsample from DSP rate to 44100 Hz by repeating each sample.
        let dsp_rate = self.dsp.sample_rate();
        let factor = (44100 / dsp_rate).max(1) as usize;
        for _ in 0..factor {
            self.pcm_out.push_sample(sample);
        }
        true
    }

    fn io_read_u8(&mut self, port: u16, cycle_count: u32) -> Option<u8> {
        let sb_off = port.wrapping_sub(self.base_port);
        match sb_off {
            // OPL status (chip 0: base+0, base+1, base+8, base+9; chip 1: base+2, base+3)
            0x00 | 0x01 | 0x02 | 0x03 | 0x08 | 0x09 => {
                return Some(self.opl.read_status(cycle_count));
            }
            // Mixer ports
            0x04 => return Some(0xFF), // index port reads back 0xFF on real HW
            0x05 => return Some(self.mixer.read_data()),
            // DSP ports
            0x0A => return Some(self.dsp.read_data()),
            0x0C => return Some(0x00), // write-buffer status: never busy
            0x0E => return Some(self.dsp.read_status()),
            0x0F => return Some(self.dsp.read_ack16()),
            _ => {}
        }
        // AdLib-compat OPL ports
        if let 0x388..=0x38B = port {
            return Some(self.opl.read_status(cycle_count));
        }
        // MPU-401 ports
        match port {
            0x330 => return Some(self.mpu.read_data()),
            0x331 => return Some(self.mpu.read_status()),
            _ => {}
        }
        self.cdrom.io_read_u8(port, cycle_count)
    }

    fn io_write_u8(&mut self, port: u16, val: u8, cycle_count: u32) -> bool {
        let sb_off = port.wrapping_sub(self.base_port);
        match sb_off {
            // OPL address ports (chip 0: base+0, base+8)
            0x00 | 0x08 => {
                self.opl.write_address(0, val, cycle_count);
                return true;
            }
            // OPL data ports (chip 0: base+1, base+9)
            0x01 | 0x09 => {
                self.opl.write_data(0, val, cycle_count);
                return true;
            }
            // OPL address port (chip 1: base+2)
            0x02 => {
                self.opl.write_address(1, val, cycle_count);
                return true;
            }
            // OPL data port (chip 1: base+3)
            0x03 => {
                self.opl.write_data(1, val, cycle_count);
                return true;
            }
            // Mixer ports
            0x04 => {
                self.mixer.write_index(val);
                return true;
            }
            0x05 => {
                self.mixer.write_data(val);
                return true;
            }
            // DSP ports
            0x06 => {
                self.dsp.write_reset_port(val);
                return true;
            }
            0x0C => {
                self.dsp.write_command_port(val);
                if let Some(byte) = self.dsp.take_direct_dac_byte() {
                    let sample = (byte as f32 - 128.0) / 128.0;
                    // Resample: push previous sample for the elapsed time, then record new one.
                    let n_hold = if let Some(prev) = self.last_dac_cycle {
                        let delta = cycle_count.wrapping_sub(prev);
                        ((delta as u64 * 44100) / self.cpu_freq).max(1) as usize
                    } else {
                        1
                    };
                    for _ in 0..n_hold {
                        self.pcm_out.push_sample(self.last_dac_sample);
                    }
                    self.last_dac_sample = sample;
                    self.last_dac_cycle = Some(cycle_count);
                }
                return true;
            }
            _ => {}
        }
        // AdLib-compat OPL ports
        match port {
            0x388 => {
                self.opl.write_address(0, val, cycle_count);
                return true;
            }
            0x389 => {
                self.opl.write_data(0, val, cycle_count);
                return true;
            }
            0x38A => {
                self.opl.write_address(1, val, cycle_count);
                return true;
            }
            0x38B => {
                self.opl.write_data(1, val, cycle_count);
                return true;
            }
            // MPU-401 ports
            0x330 => {
                self.mpu.write_data(val);
                self.midi.push_byte(val);
                return true;
            }
            0x331 => {
                self.mpu.write_command(val);
                return true;
            }
            _ => {}
        }
        self.cdrom.io_write_u8(port, val, cycle_count)
    }
}

impl SoundCard for SoundBlaster {
    fn advance_to_cycle(&mut self, cycle_count: u32) {
        self.midi.advance_to_cycle(cycle_count);
        self.opl.advance_to_cycle(cycle_count);
    }

    fn next_sample_cycle(&self) -> u32 {
        self.opl.next_sample_cycle()
    }

    fn take_dreq_request(&mut self) -> Option<(u8, bool)> {
        self.dsp.take_dreq_request().map(|asserted| (1u8, asserted))
    }
}

impl CdromController for SoundBlaster {
    fn load_disc(&mut self, disc: Box<dyn CdromBackend>) {
        self.cdrom.load_disc(disc);
    }

    fn eject_disc(&mut self) {
        self.cdrom.eject_disc();
    }

    fn take_pending_irq(&mut self) -> bool {
        if self.dsp.take_pending_irq() {
            return true;
        }
        self.cdrom.take_pending_irq()
    }

    fn irq_line(&self) -> u8 {
        self.cdrom.irq_line()
    }
}
