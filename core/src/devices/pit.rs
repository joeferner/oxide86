use std::any::Any;

use crate::{Device, devices::pc_speaker::PcSpeaker};

/// PIT oscillator frequency in Hz.
pub const PIT_FREQUENCY_HZ: u64 = 1_193_182;
/// PIT counter reload value (2^16), giving ~18.2 Hz tick rate.
pub const PIT_DIVISOR: u64 = 65_536;

pub const PIT_CHANNEL_0: u16 = 0x0040;
pub const PIT_CHANNEL_1: u16 = 0x0041;
pub const PIT_CHANNEL_2: u16 = 0x0042;
pub const PIT_CONTROL: u16 = 0x0043;
/// System Control Port B: timer 2 gate (bit 0), speaker enable (bit 1).
pub const PORT_B: u16 = 0x0061;

struct Channel {
    divisor: u16,
    /// True when waiting for the low byte (lobyte/hibyte mode).
    expect_low: bool,
    latch_buf: u8,
    /// 1=lobyte only, 2=hibyte only, 3=lobyte/hibyte
    access_mode: u8,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            divisor: 0,
            expect_low: true,
            latch_buf: 0,
            access_mode: 3,
        }
    }
}

impl Channel {
    fn write(&mut self, val: u8) -> Option<u16> {
        match self.access_mode {
            1 => Some(val as u16),
            2 => Some((val as u16) << 8),
            3 => {
                if self.expect_low {
                    self.latch_buf = val;
                    self.expect_low = false;
                    None
                } else {
                    self.expect_low = true;
                    let d = ((val as u16) << 8) | (self.latch_buf as u16);
                    Some(d)
                }
            }
            _ => None,
        }
    }
}

pub(crate) struct Pit {
    cpu_clock_speed: u32,
    cycles_per_irq: u32,
    last_irq_0_cycle_count: u32,
    ch0: Channel,
    ch2: Channel,
    /// Port B (0x0061): bit 0 = timer 2 gate, bit 1 = speaker enable.
    port_b: u8,
    pc_speaker: Box<dyn PcSpeaker>,
}

impl Pit {
    pub(crate) fn new(cpu_clock_speed: u32, pc_speaker: Box<dyn PcSpeaker>) -> Self {
        Self {
            cpu_clock_speed,
            cycles_per_irq: ((cpu_clock_speed as u64 * PIT_DIVISOR) / PIT_FREQUENCY_HZ) as u32,
            last_irq_0_cycle_count: 0,
            ch0: Channel::default(),
            ch2: Channel::default(),
            port_b: 0x00,
            pc_speaker,
        }
    }

    fn speaker_active(&self) -> bool {
        self.port_b & 0x03 == 0x03
    }

    fn ch2_freq(&self) -> f32 {
        let d = if self.ch2.divisor == 0 {
            PIT_DIVISOR
        } else {
            self.ch2.divisor as u64
        };
        PIT_FREQUENCY_HZ as f32 / d as f32
    }

    fn update_speaker(&mut self) {
        if self.speaker_active() {
            self.pc_speaker.enable(self.ch2_freq());
        } else {
            self.pc_speaker.disable();
        }
    }

    /// Returns `true` if a timer IRQ (IRQ 0) is pending and should be raised.
    pub(crate) fn take_pending_timer_irq(&mut self, cycle_count: u32) -> bool {
        let elapsed = cycle_count.wrapping_sub(self.last_irq_0_cycle_count);
        if elapsed >= self.cycles_per_irq {
            self.last_irq_0_cycle_count = cycle_count;
            true
        } else {
            false
        }
    }

    fn write_control(&mut self, val: u8) {
        let channel = (val >> 6) & 0x03;
        let access = (val >> 4) & 0x03;
        match channel {
            0 => {
                self.ch0.access_mode = access;
                self.ch0.expect_low = access == 3;
            }
            2 => {
                self.ch2.access_mode = access;
                self.ch2.expect_low = access == 3;
            }
            _ => {}
        }
    }
}

impl Device for Pit {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.last_irq_0_cycle_count = 0;
        self.cycles_per_irq =
            ((self.cpu_clock_speed as u64 * PIT_DIVISOR) / PIT_FREQUENCY_HZ) as u32;
        self.ch0 = Channel::default();
        self.ch2 = Channel::default();
        self.port_b = 0x00;
        self.pc_speaker.disable();
    }

    fn memory_read_u8(&self, _addr: usize) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16) -> Option<u8> {
        match port {
            PIT_CHANNEL_0 | PIT_CHANNEL_1 | PIT_CHANNEL_2 | PIT_CONTROL => Some(0xFF),
            PORT_B => {
                // Bit 5: timer 2 output (high if channel 2 running and gate open).
                let timer2_out = if self.port_b & 0x01 != 0 { 0x20 } else { 0x00 };
                Some((self.port_b & 0x0F) | timer2_out)
            }
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8) -> bool {
        match port {
            PIT_CONTROL => {
                self.write_control(val);
                true
            }
            PIT_CHANNEL_0 => {
                if let Some(divisor) = self.ch0.write(val) {
                    self.ch0.divisor = divisor;
                    // divisor == 0 means 65536 (full counter wrap)
                    let d = if divisor == 0 {
                        PIT_DIVISOR
                    } else {
                        divisor as u64
                    };
                    self.cycles_per_irq =
                        ((self.cpu_clock_speed as u64 * d) / PIT_FREQUENCY_HZ) as u32;
                }
                true
            }
            PIT_CHANNEL_1 => true,
            PIT_CHANNEL_2 => {
                if let Some(divisor) = self.ch2.write(val) {
                    self.ch2.divisor = divisor;
                    self.update_speaker();
                }
                true
            }
            PORT_B => {
                self.port_b = val;
                self.update_speaker();
                true
            }
            _ => false,
        }
    }
}
