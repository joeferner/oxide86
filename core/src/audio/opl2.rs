/// Hand-rolled OPL2 (Yamaha YM3812) FM synthesis emulator.
///
/// Supports 9 FM channels, 2 operators each, 4 waveforms, ADSR envelopes,
/// tremolo/vibrato LFOs, and timer registers for AdLib detection.
///
/// I/O ports:
///   0x388 — Address port (write) / Status register (read)
///   0x389 — Data port (write) / Status register (read)
use crate::audio::adlib::ADLIB_SAMPLE_RATE;
use std::sync::OnceLock;

// --- Constants ---

/// OPL2 internal sample rate (Hz)
const OPL_RATE: u64 = 49716;

/// Phase accumulator size: 2^20
const PHASE_MASK: u32 = 0xFFFFF;

/// Operator-to-channel mapping: OPL_MOD_SLOT[ch] = modulator slot index
const OPL_MOD_SLOT: [usize; 9] = [0, 1, 2, 6, 7, 8, 12, 13, 14];
/// OPL_CAR_SLOT[ch] = carrier slot index
const OPL_CAR_SLOT: [usize; 9] = [3, 4, 5, 9, 10, 11, 15, 16, 17];

/// Register offset → operator slot. Returns None for invalid offsets.
fn offset_to_slot(offset: u8) -> Option<usize> {
    match offset {
        0x00 => Some(0),
        0x01 => Some(1),
        0x02 => Some(2),
        0x03 => Some(3),
        0x04 => Some(4),
        0x05 => Some(5),
        0x08 => Some(6),
        0x09 => Some(7),
        0x0A => Some(8),
        0x0B => Some(9),
        0x0C => Some(10),
        0x0D => Some(11),
        0x10 => Some(12),
        0x11 => Some(13),
        0x12 => Some(14),
        0x13 => Some(15),
        0x14 => Some(16),
        0x15 => Some(17),
        _ => None,
    }
}

/// Frequency multiplier table: FREQ_MULT[multiple] = phase_step multiplier * 2
/// (×2 so we can represent 0.5× for multiple=0 as integer 1 then halve).
const FREQ_MULT: [u32; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];

/// Vibrato depth: ±7 cents (simplified)
const VIBRATO_CENTS: f32 = 0.004; // ±0.4% frequency

// --- Precomputed sine table ---

static SINE_TABLE: OnceLock<[f32; 1024]> = OnceLock::new();

fn get_sine_table() -> &'static [f32; 1024] {
    SINE_TABLE.get_or_init(|| {
        let mut t = [0.0f32; 1024];
        for (i, v) in t.iter_mut().enumerate() {
            *v = (2.0 * std::f32::consts::PI * i as f32 / 1024.0).sin();
        }
        t
    })
}

/// Sample the waveform at the given 20-bit phase.
fn waveform_sample(phase: u32, waveform: u8, waveform_enable: bool) -> f32 {
    let table = get_sine_table();
    let idx = ((phase >> 10) & 0x3FF) as usize; // top 10 bits → 0..1023

    if !waveform_enable {
        return table[idx];
    }

    match waveform & 0x03 {
        0 => table[idx], // Full sine
        1 => {
            // Positive half-wave only (negative half = 0)
            if idx < 512 { table[idx] } else { 0.0 }
        }
        2 => table[idx].abs(), // Absolute sine (full rectified)
        _ => {
            // Waveform 3: pulse sine — positive first quarter, zero elsewhere
            let quarter = idx / 256;
            if quarter.is_multiple_of(2) {
                table[idx % 256].abs() // Mirror first quarter positively
            } else {
                0.0
            }
        }
    }
}

// --- Envelope ---

#[derive(Clone, Copy, PartialEq)]
enum EnvPhase {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

/// Returns the envelope increment per OPL sample for a given rate (0-15).
/// Rate 0 = no change, rate 15 = instant.
fn env_increment(rate: u8) -> u32 {
    match rate {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 5,
        5 => 7,
        6 => 10,
        7 => 14,
        8 => 20,
        9 => 28,
        10 => 40,
        11 => 56,
        12 => 80,
        13 => 112,
        14 => 160,
        _ => 1023, // rate=15: instant
    }
}

// --- Operator ---

#[derive(Clone)]
struct Operator {
    // Register-backed fields
    tremolo: bool,
    vibrato: bool,
    sustain_mode: bool, // EG type: true = hold sustain until key-off
    ksr: bool,          // Key scale rate
    multiple: u8,       // Frequency multiplier index (0-15)
    total_level: u8,    // Attenuation 0-63 (0=loudest, 63=silent)
    attack: u8,         // Attack rate 0-15
    decay: u8,          // Decay rate 0-15
    sustain: u8,        // Sustain level 0-15 (15=loudest retained)
    release: u8,        // Release rate 0-15
    waveform: u8,       // Waveform 0-3

    // Envelope state
    env_phase: EnvPhase,
    env_level: u32, // 0 = loudest, 1023 = silent

    // Phase accumulator (20-bit)
    phase_acc: u32,
    phase_step: u32,

    // Vibrato phase offset (added to phase when vibrato enabled)
    vibrato_factor: f32,
}

impl Default for Operator {
    fn default() -> Self {
        Self {
            tremolo: false,
            vibrato: false,
            sustain_mode: false,
            ksr: false,
            multiple: 0,
            total_level: 63, // Start at maximum attenuation (silent)
            attack: 0,
            decay: 0,
            sustain: 0,
            release: 0,
            waveform: 0,
            env_phase: EnvPhase::Off,
            env_level: 1023,
            phase_acc: 0,
            phase_step: 0,
            vibrato_factor: 1.0,
        }
    }
}

impl Operator {
    fn key_on(&mut self) {
        self.env_phase = EnvPhase::Attack;
        self.env_level = 1023;
        self.phase_acc = 0; // Reset phase on key-on
    }

    fn key_off(&mut self) {
        if self.env_phase != EnvPhase::Off {
            self.env_phase = EnvPhase::Release;
        }
    }

    fn advance_envelope(&mut self) {
        match self.env_phase {
            EnvPhase::Attack => {
                if self.attack == 0 {
                    // Rate 0: never attack (stay at 1023 until key-off)
                    return;
                }
                if self.attack >= 15 {
                    self.env_level = 0;
                    self.env_phase = EnvPhase::Decay;
                    return;
                }
                // Exponential (curved) attack mirroring OPL2 hardware.
                // Real OPL2: increment ∝ ~eg_rout (Nuked-OPL3 formula), meaning large
                // steps when quiet (env_level high) and small steps near full volume
                // (env_level → 0).  We approximate this by making inc ∝ env_level itself:
                // a rate-derived shift controls the overall speed, then multiplying by
                // env_level/1023 bends the straight line into the hardware-like curve.
                let shift: u32 = match self.attack {
                    1 => 14,
                    2 => 12,
                    3 => 11,
                    4 => 10,
                    5 => 9,
                    6 => 8,
                    7 => 7,
                    8 => 6,
                    9 => 5,
                    10 => 4,
                    11 => 3,
                    12 => 2,
                    13 => 1,
                    _ => 0, // 14: shift=0 → very fast
                };
                // inc = env_level >> shift, guaranteed ≥ 1 so we always make progress.
                let inc = (self.env_level >> shift).max(1);
                self.env_level = self.env_level.saturating_sub(inc);
                if self.env_level == 0 {
                    self.env_phase = EnvPhase::Decay;
                }
            }
            EnvPhase::Decay => {
                // SL=0 → sustain at peak (env_level 0); SL=15 → sustain near-silent (env_level 1020)
                let sustain_level = self.sustain as u32 * 68; // 0..1020
                let inc = env_increment(self.decay);
                self.env_level = (self.env_level + inc).min(sustain_level);
                if self.env_level >= sustain_level {
                    self.env_level = sustain_level;
                    self.env_phase = if self.sustain_mode {
                        EnvPhase::Sustain
                    } else {
                        EnvPhase::Release
                    };
                }
            }
            EnvPhase::Sustain => {} // Hold until key-off
            EnvPhase::Release => {
                if self.release == 0 {
                    return; // Rate 0: hold current level
                }
                let inc = env_increment(self.release);
                self.env_level = (self.env_level + inc).min(1023);
                if self.env_level >= 1023 {
                    self.env_level = 1023;
                    self.env_phase = EnvPhase::Off;
                }
            }
            EnvPhase::Off => {}
        }
    }

    /// Advance phase accumulator by one sample (vibrato applied).
    fn advance_phase_acc(&mut self) {
        let step = (self.phase_step as f32 * self.vibrato_factor) as u32;
        self.phase_acc = self.phase_acc.wrapping_add(step) & PHASE_MASK;
    }

    /// Sample the waveform at `phase` and apply the current envelope volume.
    /// Does NOT advance envelope or phase — call those first.
    /// Used by rhythm synthesis where the phase is computed externally.
    fn sample_at_phase(&self, phase: u32, waveform_enable: bool, tremolo_amp: f32) -> i32 {
        if self.env_phase == EnvPhase::Off {
            return 0;
        }
        let sample = waveform_sample(phase & PHASE_MASK, self.waveform, waveform_enable);
        // OPL2 hardware uses logarithmic (dB-based) attenuation.
        // TL: 6-bit register, 0.75 dB/step → 0–47.25 dB total range.
        // Envelope: 0–1023 mapped to the same 0–47.25 dB scale.
        let tl_db = self.total_level as f32 * 0.75;
        let env_db = self.env_level as f32 * (47.25 / 1023.0);
        let total_db = tl_db + env_db;
        let volume = if total_db >= 96.0 {
            0.0
        } else {
            10.0f32.powf(-total_db / 20.0)
        };
        // Tremolo: ±4.8 dB (hardware spec)
        let volume = if self.tremolo {
            volume * 10.0f32.powf(-tremolo_amp * 4.8 / 20.0)
        } else {
            volume
        };
        (sample * volume * 32767.0) as i32
    }

    /// Compute one sample output given a phase modulation input.
    /// Returns a value in the range -32767..32767.
    fn calc_output(&mut self, phase_mod: i32, waveform_enable: bool, tremolo_amp: f32) -> i32 {
        if self.env_phase == EnvPhase::Off {
            return 0;
        }
        self.advance_envelope();
        self.advance_phase_acc();
        let phase = self.phase_acc.wrapping_add(phase_mod as u32) & PHASE_MASK;
        self.sample_at_phase(phase, waveform_enable, tremolo_amp)
    }
}

// --- Channel ---

#[derive(Default, Clone)]
struct Channel {
    fnum: u16, // 10-bit frequency number
    block: u8, // Block (octave) 0-7
    key_on: bool,
    feedback: u8,     // Modulator self-feedback level 0-7 (0=none)
    algorithm: u8,    // 0=FM, 1=Additive
    fb_buf: [u32; 2], // Last two modulator outputs for feedback (fixed-point)
}

// --- OPL2 ---

pub struct Opl2 {
    regs: [u8; 256],
    operators: [Operator; 18],
    channels: [Channel; 9],

    // Global flags
    waveform_enable: bool,

    // LFO state
    tremolo_phase: u32, // 0..1023
    vibrato_phase: u32, // 0..1023
    lfo_counter: u32,   // Counts OPL samples for LFO advancement

    // Timer registers
    pending_address: u8,
    timer1_value: u8,
    timer2_value: u8,
    timer_control: u8,
    timer1_counter: u32,
    timer2_counter: u32,
    pub status: u8, // Readable at port 0x388

    // Sample generation accumulators
    cycle_acc: u64,    // CPU cycles → OPL samples
    resample_acc: u64, // OPL samples → target sample rate

    // CPU clock frequency — determines how many OPL samples each CPU cycle produces
    // and scales timer tick thresholds to match the actual clock speed.
    cpu_freq: u64,
    /// Timer 1 fires every (256 - value) * 80 µs; threshold in CPU cycles at cpu_freq.
    timer1_cycles_per_tick: u32,
    /// Timer 2 fires every (256 - value) * 320 µs; threshold in CPU cycles at cpu_freq.
    timer2_cycles_per_tick: u32,

    // Rhythm mode (register 0xBD)
    rhythm_mode: bool,
    rhythm_bd_on: bool, // Bass drum
    rhythm_sd_on: bool, // Snare drum
    rhythm_tt_on: bool, // Tom-tom
    rhythm_cy_on: bool, // Cymbal
    rhythm_hh_on: bool, // Hi-hat
    /// 23-bit LFSR for noise synthesis (hi-hat, snare, cymbal)
    noise_lfsr: u32,
}

impl Opl2 {
    pub fn new(cpu_freq: u64) -> Self {
        // Pre-init the sine table so first sample is cheap
        get_sine_table();

        // Timer 1: 80 µs period; Timer 2: 320 µs period — in CPU cycles at cpu_freq.
        let timer1_cycles_per_tick = (80e-6 * cpu_freq as f64).round() as u32;
        let timer2_cycles_per_tick = (320e-6 * cpu_freq as f64).round() as u32;

        Self {
            regs: [0u8; 256],
            operators: std::array::from_fn(|_| Operator::default()),
            channels: std::array::from_fn(|_| Channel::default()),
            waveform_enable: false,
            tremolo_phase: 0,
            vibrato_phase: 0,
            lfo_counter: 0,
            pending_address: 0,
            timer1_value: 0,
            timer2_value: 0,
            timer_control: 0,
            timer1_counter: 0,
            timer2_counter: 0,
            status: 0,
            cycle_acc: 0,
            resample_acc: 0,
            cpu_freq,
            timer1_cycles_per_tick,
            timer2_cycles_per_tick,
            rhythm_mode: false,
            rhythm_bd_on: false,
            rhythm_sd_on: false,
            rhythm_tt_on: false,
            rhythm_cy_on: false,
            rhythm_hh_on: false,
            noise_lfsr: 0x1,
        }
    }

    // --- I/O port handlers ---

    pub fn write_address(&mut self, addr: u8) {
        self.pending_address = addr;
    }

    pub fn write_data(&mut self, value: u8) {
        let addr = self.pending_address;
        self.regs[addr as usize] = value;
        self.dispatch_register(addr, value);
    }

    pub fn read_status(&self) -> u8 {
        self.status
    }

    // --- Register dispatch ---

    fn dispatch_register(&mut self, addr: u8, value: u8) {
        match addr {
            0x01 => {
                self.waveform_enable = (value & 0x20) != 0;
                log::debug!("OPL2: waveform_enable={}", self.waveform_enable);
            }
            0x02 => {
                self.timer1_value = value;
            }
            0x03 => {
                self.timer2_value = value;
            }
            0x04 => {
                self.handle_timer_control(value);
            }
            0x08 => {} // CSW / Note select — store only
            0x20..=0x35 => {
                if let Some(slot) = offset_to_slot(addr - 0x20) {
                    let op = &mut self.operators[slot];
                    op.tremolo = (value & 0x80) != 0;
                    op.vibrato = (value & 0x40) != 0;
                    op.sustain_mode = (value & 0x20) != 0;
                    op.ksr = (value & 0x10) != 0;
                    op.multiple = value & 0x0F;
                }
            }
            0x40..=0x55 => {
                if let Some(slot) = offset_to_slot(addr - 0x40) {
                    self.operators[slot].total_level = value & 0x3F;
                    // KSL in bits 7:6 — ignored in v1 (ksl=0 means no rolloff)
                }
            }
            0x60..=0x75 => {
                if let Some(slot) = offset_to_slot(addr - 0x60) {
                    self.operators[slot].attack = (value >> 4) & 0x0F;
                    self.operators[slot].decay = value & 0x0F;
                }
            }
            0x80..=0x95 => {
                if let Some(slot) = offset_to_slot(addr - 0x80) {
                    self.operators[slot].sustain = (value >> 4) & 0x0F;
                    self.operators[slot].release = value & 0x0F;
                }
            }
            0xA0..=0xA8 => {
                let ch = (addr - 0xA0) as usize;
                if ch < 9 {
                    self.channels[ch].fnum = (self.channels[ch].fnum & 0x300) | (value as u16);
                    self.update_channel_freq(ch);
                }
            }
            0xB0..=0xB8 => {
                let ch = (addr - 0xB0) as usize;
                if ch < 9 {
                    let new_key_on = (value & 0x20) != 0;
                    let block = (value >> 2) & 0x07;
                    let fnum_hi = (value & 0x03) as u16;
                    self.channels[ch].block = block;
                    self.channels[ch].fnum = (self.channels[ch].fnum & 0xFF) | (fnum_hi << 8);
                    self.update_channel_freq(ch);

                    let was_key_on = self.channels[ch].key_on;
                    self.channels[ch].key_on = new_key_on;

                    let mod_slot = OPL_MOD_SLOT[ch];
                    let car_slot = OPL_CAR_SLOT[ch];

                    if new_key_on && !was_key_on {
                        // Key-on rising edge
                        self.channels[ch].fb_buf = [0; 2];
                        self.operators[mod_slot].key_on();
                        self.operators[car_slot].key_on();
                        log::debug!(
                            "OPL2: ch{} key-on fnum={} block={}",
                            ch,
                            self.channels[ch].fnum,
                            block
                        );
                    } else if !new_key_on && was_key_on {
                        // Key-off falling edge
                        self.operators[mod_slot].key_off();
                        self.operators[car_slot].key_off();
                    }
                }
            }
            0xBD => {
                let new_rhythm = (value & 0x20) != 0;

                if new_rhythm && !self.rhythm_mode {
                    // Entering rhythm mode: silence channels 6-8 as melodic channels
                    for ch in 6..9 {
                        self.channels[ch].key_on = false;
                        self.operators[OPL_MOD_SLOT[ch]].key_off();
                        self.operators[OPL_CAR_SLOT[ch]].key_off();
                    }
                } else if !new_rhythm && self.rhythm_mode {
                    // Leaving rhythm mode: key off all rhythm instruments
                    self.operators[OPL_MOD_SLOT[6]].key_off();
                    self.operators[OPL_CAR_SLOT[6]].key_off();
                    self.operators[OPL_MOD_SLOT[7]].key_off();
                    self.operators[OPL_CAR_SLOT[7]].key_off();
                    self.operators[OPL_MOD_SLOT[8]].key_off();
                    self.operators[OPL_CAR_SLOT[8]].key_off();
                }

                self.rhythm_mode = new_rhythm;

                if self.rhythm_mode {
                    // Bass drum: modulator=op12 (mod_slot[6]), carrier=op15 (car_slot[6])
                    let bd = (value & 0x10) != 0;
                    if bd && !self.rhythm_bd_on {
                        self.operators[OPL_MOD_SLOT[6]].key_on();
                        self.operators[OPL_CAR_SLOT[6]].key_on();
                    } else if !bd && self.rhythm_bd_on {
                        self.operators[OPL_MOD_SLOT[6]].key_off();
                        self.operators[OPL_CAR_SLOT[6]].key_off();
                    }
                    self.rhythm_bd_on = bd;

                    // Snare drum: carrier=op16 (car_slot[7])
                    let sd = (value & 0x08) != 0;
                    if sd && !self.rhythm_sd_on {
                        self.operators[OPL_CAR_SLOT[7]].key_on();
                    } else if !sd && self.rhythm_sd_on {
                        self.operators[OPL_CAR_SLOT[7]].key_off();
                    }
                    self.rhythm_sd_on = sd;

                    // Tom-tom: modulator=op14 (mod_slot[8])
                    let tt = (value & 0x04) != 0;
                    if tt && !self.rhythm_tt_on {
                        self.operators[OPL_MOD_SLOT[8]].key_on();
                    } else if !tt && self.rhythm_tt_on {
                        self.operators[OPL_MOD_SLOT[8]].key_off();
                    }
                    self.rhythm_tt_on = tt;

                    // Cymbal: carrier=op17 (car_slot[8])
                    let cy = (value & 0x02) != 0;
                    if cy && !self.rhythm_cy_on {
                        self.operators[OPL_CAR_SLOT[8]].key_on();
                    } else if !cy && self.rhythm_cy_on {
                        self.operators[OPL_CAR_SLOT[8]].key_off();
                    }
                    self.rhythm_cy_on = cy;

                    // Hi-hat: modulator=op13 (mod_slot[7])
                    let hh = (value & 0x01) != 0;
                    if hh && !self.rhythm_hh_on {
                        self.operators[OPL_MOD_SLOT[7]].key_on();
                    } else if !hh && self.rhythm_hh_on {
                        self.operators[OPL_MOD_SLOT[7]].key_off();
                    }
                    self.rhythm_hh_on = hh;
                }
            }
            0xC0..=0xC8 => {
                let ch = (addr - 0xC0) as usize;
                if ch < 9 {
                    self.channels[ch].feedback = (value >> 1) & 0x07;
                    self.channels[ch].algorithm = value & 0x01;
                }
            }
            0xE0..=0xF5 => {
                if let Some(slot) = offset_to_slot(addr - 0xE0) {
                    self.operators[slot].waveform = value & 0x03;
                }
            }
            _ => {
                log::trace!("OPL2: unhandled register 0x{:02X} = 0x{:02X}", addr, value);
            }
        }
    }

    fn update_channel_freq(&mut self, ch: usize) {
        let fnum = self.channels[ch].fnum;
        let block = self.channels[ch].block;

        let mod_slot = OPL_MOD_SLOT[ch];
        let car_slot = OPL_CAR_SLOT[ch];

        self.operators[mod_slot].phase_step =
            compute_phase_step(fnum, block, self.operators[mod_slot].multiple);
        self.operators[car_slot].phase_step =
            compute_phase_step(fnum, block, self.operators[car_slot].multiple);
    }

    // --- Timer handling ---

    fn handle_timer_control(&mut self, value: u8) {
        if value & 0x80 != 0 {
            // Reset timer flags (bit 7 = IRQ reset)
            self.status = 0;
            return;
        }
        self.timer_control = value;
        // Restart counters when start bits are set
        if value & 0x01 != 0 {
            self.timer1_counter = 0;
        }
        if value & 0x02 != 0 {
            self.timer2_counter = 0;
        }
    }

    /// Advance timers by the given CPU cycle count.
    /// Called every step whether or not audio output is active.
    pub fn advance_timers(&mut self, cpu_cycles: u64) {
        let cycles = cpu_cycles as u32;

        // Timer 1: bit 0 of timer_control starts it; bit 6 masks the status flag
        if self.timer_control & 0x01 != 0 {
            self.timer1_counter += cycles;
            let ticks = (256 - self.timer1_value as u32).max(1);
            let threshold = ticks * self.timer1_cycles_per_tick;
            if self.timer1_counter >= threshold {
                self.timer1_counter = 0;
                if self.timer_control & 0x40 == 0 {
                    // Not masked → set status bits
                    self.status |= 0xC0; // bit 7 (IRQ) + bit 6 (Timer 1 expired)
                }
            }
        }

        // Timer 2: bit 1 starts it; bit 5 masks status
        if self.timer_control & 0x02 != 0 {
            self.timer2_counter += cycles;
            let ticks = (256 - self.timer2_value as u32).max(1);
            let threshold = ticks * self.timer2_cycles_per_tick;
            if self.timer2_counter >= threshold {
                self.timer2_counter = 0;
                if self.timer_control & 0x20 == 0 {
                    self.status |= 0xA0; // bit 7 (IRQ) + bit 5 (Timer 2 expired)
                }
            }
        }
    }

    // --- LFO ---

    fn advance_lfos(&mut self) {
        // Tremolo: ~3.7 Hz at 49716 Hz → ~13436 samples per cycle
        // Vibrato: ~6.1 Hz → ~8150 samples per cycle
        self.lfo_counter += 1;
        if self.lfo_counter >= 13 {
            // Advance every ~13 OPL samples (≈3814 Hz, close enough)
            self.lfo_counter = 0;
            self.tremolo_phase = (self.tremolo_phase + 1) % 1024;
            self.vibrato_phase = (self.vibrato_phase + 2) % 1024;

            // Update vibrato factors for each operator
            let vib_table = get_sine_table();
            let vib_val = vib_table[self.vibrato_phase as usize] * VIBRATO_CENTS;
            for op in self.operators.iter_mut() {
                if op.vibrato {
                    op.vibrato_factor = 1.0 + vib_val;
                }
            }
        }
    }

    fn tremolo_amp(&self) -> f32 {
        let table = get_sine_table();
        table[self.tremolo_phase as usize].abs()
    }

    // --- Sample generation ---

    fn generate_one_sample(&mut self) -> f32 {
        let tremolo = self.tremolo_amp();
        let waveform_enable = self.waveform_enable;
        let mut mix: i32 = 0;

        // In rhythm mode, channels 6-8 are repurposed for rhythm instruments
        let melodic_channels = if self.rhythm_mode { 6 } else { 9 };

        for ch in 0..melodic_channels {
            let mod_slot = OPL_MOD_SLOT[ch];
            let car_slot = OPL_CAR_SLOT[ch];

            // Skip completely silent channels
            if self.operators[mod_slot].env_phase == EnvPhase::Off
                && self.operators[car_slot].env_phase == EnvPhase::Off
            {
                continue;
            }

            // Feedback self-modulation on modulator
            let fb = self.channels[ch].feedback;
            let fb_mod: i32 = if fb > 0 {
                let avg = ((self.channels[ch].fb_buf[0] as i32)
                    + (self.channels[ch].fb_buf[1] as i32))
                    / 2;
                avg >> (9 - fb as i32)
            } else {
                0
            };

            // Modulator output
            // Temporarily remove the operator from the array to avoid aliasing
            let mod_out = {
                let op = &mut self.operators[mod_slot];
                op.calc_output(fb_mod, waveform_enable, tremolo)
            };

            // Update feedback buffer
            self.channels[ch].fb_buf[1] = self.channels[ch].fb_buf[0];
            self.channels[ch].fb_buf[0] = mod_out as u32;

            // Carrier output
            let car_out = {
                let op = &mut self.operators[car_slot];
                if self.channels[ch].algorithm == 0 {
                    // FM: modulator output modulates carrier phase.
                    // Hardware-accurate depth: in Nuked-OPL3, full-volume modulator output
                    // (~±8192 units) is added directly to the 10-bit phase index (1024
                    // entries). Our full-volume output is ±32767; << 8 gives ≈8192 effective
                    // table-index shifts (32767>>10 ≈ 31; 31<<8 ≈ 8192), matching hardware.
                    let phase_mod = mod_out << 8;
                    op.calc_output(phase_mod, waveform_enable, tremolo)
                } else {
                    // Additive: independent carrier
                    op.calc_output(0, waveform_enable, tremolo)
                }
            };

            // Mix: FM mode → carrier only; Additive → both
            if self.channels[ch].algorithm == 0 {
                mix += car_out;
            } else {
                mix += mod_out / 2 + car_out / 2;
            }
        }

        // Rhythm mode: channels 6-8 become rhythm instruments.
        // Hi-hat, snare, and cymbal use phase-based synthesis (Nuked-OPL3 approach):
        // specific bits of the hi-hat (slot 13) and cymbal (slot 17) phase accumulators
        // are XOR'd together to produce the metallic/noisy character, rather than
        // mixing raw noise amplitude with operator output.
        if self.rhythm_mode {
            // Advance noise LFSR (23-bit, Nuked-OPL3: taps at bits 14 and 0, shift right)
            let n_bit = ((self.noise_lfsr >> 14) ^ self.noise_lfsr) & 1;
            self.noise_lfsr = (self.noise_lfsr >> 1) | (n_bit << 22);
            let noise_bit = self.noise_lfsr & 1;

            // Bass drum (ch 6): standard FM pair, same as melodic channels.
            {
                let mod_slot = OPL_MOD_SLOT[6]; // 12
                let car_slot = OPL_CAR_SLOT[6]; // 15
                if self.operators[mod_slot].env_phase != EnvPhase::Off
                    || self.operators[car_slot].env_phase != EnvPhase::Off
                {
                    let fb = self.channels[6].feedback;
                    let fb_mod: i32 = if fb > 0 {
                        let avg = ((self.channels[6].fb_buf[0] as i32)
                            + (self.channels[6].fb_buf[1] as i32))
                            / 2;
                        avg >> (9 - fb as i32)
                    } else {
                        0
                    };
                    let mod_out = {
                        let op = &mut self.operators[mod_slot];
                        op.calc_output(fb_mod, waveform_enable, tremolo)
                    };
                    self.channels[6].fb_buf[1] = self.channels[6].fb_buf[0];
                    self.channels[6].fb_buf[0] = mod_out as u32;
                    let car_out = {
                        let op = &mut self.operators[car_slot];
                        if self.channels[6].algorithm == 0 {
                            op.calc_output(mod_out << 8, waveform_enable, tremolo)
                        } else {
                            op.calc_output(0, waveform_enable, tremolo)
                        }
                    };
                    if self.channels[6].algorithm == 0 {
                        mix += car_out;
                    } else {
                        mix += mod_out / 2 + car_out / 2;
                    }
                }
            }

            // Pre-advance hi-hat (slot 13) and cymbal/tc (slot 17) phase accumulators.
            // Phase bits from both operators feed the rm_xor formula, so both must be
            // advanced before any output is computed.  Envelope is also advanced here
            // (only when not Off) so that sample_at_phase sees the up-to-date level.
            {
                let op = &mut self.operators[OPL_MOD_SLOT[7]]; // hi-hat
                op.advance_phase_acc();
                if op.env_phase != EnvPhase::Off {
                    op.advance_envelope();
                }
            }
            {
                let op = &mut self.operators[OPL_CAR_SLOT[8]]; // cymbal/tc
                op.advance_phase_acc();
                if op.env_phase != EnvPhase::Off {
                    op.advance_envelope();
                }
            }
            // Snare also needs phase advanced (uses hh_bit8 from hi-hat, computed below)
            {
                let op = &mut self.operators[OPL_CAR_SLOT[7]]; // snare
                op.advance_phase_acc();
                if op.env_phase != EnvPhase::Off {
                    op.advance_envelope();
                }
            }

            // Extract phase bits from hi-hat (slot 13) and cymbal (slot 17).
            let hh_phase = self.operators[OPL_MOD_SLOT[7]].phase_acc;
            let tc_phase = self.operators[OPL_CAR_SLOT[8]].phase_acc;
            let hh_bit2 = (hh_phase >> 2) & 1;
            let hh_bit3 = (hh_phase >> 3) & 1;
            let hh_bit7 = (hh_phase >> 7) & 1;
            let hh_bit8 = (hh_phase >> 8) & 1;
            let tc_bit3 = (tc_phase >> 3) & 1;
            let tc_bit5 = (tc_phase >> 5) & 1;

            // rm_xor: XOR combination that creates the metallic phase relationship.
            let rm_xor = ((hh_bit2 ^ hh_bit7) | (hh_bit3 ^ tc_bit5) | (tc_bit3 ^ tc_bit5)) & 1;

            // Compute 10-bit phase table indices (matching Nuked-OPL3 formulas).
            // Hi-hat: rm_xor selects the base half-cycle; noise selects offset within it.
            let hh_idx = (rm_xor << 9) | if rm_xor != noise_bit { 0xd0 } else { 0x34 };
            // Snare: hh_bit8 selects base, XOR with noise picks fine position.
            let sd_idx = (hh_bit8 << 9) | ((hh_bit8 ^ noise_bit) << 8);
            // Cymbal: rm_xor selects half-cycle, fixed 0x80 offset.
            let tc_idx = (rm_xor << 9) | 0x80;

            // Convert 10-bit indices to 20-bit phase space.
            // waveform_sample() reads bits 19:10 of the 20-bit phase as the table index.
            let hh_phase_ov = (hh_idx << 10) & PHASE_MASK;
            let sd_phase_ov = (sd_idx << 10) & PHASE_MASK;
            let tc_phase_ov = (tc_idx << 10) & PHASE_MASK;

            // Hi-hat output at override phase (envelope/phase already advanced above)
            if self.operators[OPL_MOD_SLOT[7]].env_phase != EnvPhase::Off {
                mix += self.operators[OPL_MOD_SLOT[7]].sample_at_phase(
                    hh_phase_ov,
                    waveform_enable,
                    tremolo,
                );
            }

            // Snare drum output at override phase
            if self.operators[OPL_CAR_SLOT[7]].env_phase != EnvPhase::Off {
                mix += self.operators[OPL_CAR_SLOT[7]].sample_at_phase(
                    sd_phase_ov,
                    waveform_enable,
                    tremolo,
                );
            }

            // Tom-tom (op 14 = mod_slot[8]): pure FM tone, normal calc_output.
            if self.operators[OPL_MOD_SLOT[8]].env_phase != EnvPhase::Off {
                mix += self.operators[OPL_MOD_SLOT[8]].calc_output(0, waveform_enable, tremolo);
            }

            // Cymbal output at override phase (envelope/phase already advanced above)
            if self.operators[OPL_CAR_SLOT[8]].env_phase != EnvPhase::Off {
                mix += self.operators[OPL_CAR_SLOT[8]].sample_at_phase(
                    tc_phase_ov,
                    waveform_enable,
                    tremolo,
                );
            }
        }

        self.advance_lfos();

        // Normalize to [-1.0, 1.0].  Allow up to ~3 channels at full amplitude
        // simultaneously before hard-clipping, which avoids distortion on chords.
        mix.clamp(-32767 * 3, 32767 * 3) as f32 / (32767.0 * 3.0)
    }

    /// Advance chip state by `cpu_cycles` and append resampled PCM to `out`.
    /// Also advances timers. Output rate is ADLIB_SAMPLE_RATE Hz.
    pub fn generate_samples(&mut self, cpu_cycles: u64, out: &mut Vec<f32>) {
        // Advance timers first (always needed for detection)
        self.advance_timers(cpu_cycles);

        // How many OPL samples does `cpu_cycles` correspond to?
        self.cycle_acc += cpu_cycles * OPL_RATE;
        let opl_samples = self.cycle_acc / self.cpu_freq;
        self.cycle_acc %= self.cpu_freq;

        for _ in 0..opl_samples {
            let sample = self.generate_one_sample();

            // Simple linear downsampler: emit one output sample every
            // OPL_RATE / ADLIB_SAMPLE_RATE ≈ 1.127 OPL samples
            self.resample_acc += ADLIB_SAMPLE_RATE as u64;
            if self.resample_acc >= OPL_RATE {
                self.resample_acc -= OPL_RATE;
                out.push(sample);
            }
        }
    }

    /// Advance timers only (without generating samples).
    /// Called when no audio output is configured.
    pub fn advance_timers_only(&mut self, cpu_cycles: u64) {
        self.advance_timers(cpu_cycles);
    }
}

/// Compute the phase step for an operator given channel freq params.
fn compute_phase_step(fnum: u16, block: u8, multiple: u8) -> u32 {
    // phase_step = (fnum << block) * FREQ_MULT[multiple] / 2
    // Phase accumulator is 2^20 steps per waveform cycle.
    // At OPL_RATE samples/sec: one cycle = OPL_RATE / freq samples.
    // freq_hz = fnum * OPL_RATE * 2^block / 2^20 * mult_real
    //         (where mult_real = FREQ_MULT[m] / 2)
    // phase_step = freq_hz * 2^20 / OPL_RATE = fnum * 2^block * FREQ_MULT[m] / 2
    let mult = FREQ_MULT[multiple as usize];
    ((fnum as u32) << (block as u32)) * mult / 2
}
