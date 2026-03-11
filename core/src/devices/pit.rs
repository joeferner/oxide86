use std::any::Any;
use std::cell::Cell;

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
    /// True when waiting for the low byte (lo/hi mode).
    expect_low: bool,
    latch_buf: u8,
    /// 1=lo only, 2=hi only, 3=lo/hi
    access_mode: u8,
    /// Latched counter value from a counter-latch command.
    latched: Cell<Option<u16>>,
    /// True when the high byte of the latch should be returned next.
    latch_read_high: Cell<bool>,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            divisor: 0,
            expect_low: true,
            latch_buf: 0,
            access_mode: 3,
            latched: Cell::new(None),
            latch_read_high: Cell::new(false),
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

    /// Returns the channel 0 counter value based on emulated CPU cycles.
    fn counter_value_ch0(&self, cycle_count: u32) -> u16 {
        let d = if self.ch0.divisor == 0 {
            PIT_DIVISOR
        } else {
            self.ch0.divisor as u64
        };
        let ticks =
            (cycle_count as u64).wrapping_mul(PIT_FREQUENCY_HZ) / self.cpu_clock_speed as u64;
        (d - ticks % d) as u16
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
            let freq = self.ch2_freq();
            if freq <= 20_000.0 {
                self.pc_speaker.enable(freq);
            } else {
                // Above audible range: real speaker membrane can't respond,
                // so it produces no sound (effectively DC).
                self.pc_speaker.disable();
            }
        } else {
            self.pc_speaker.disable();
        }
    }

    /// Returns `true` if a timer IRQ (IRQ 0) is pending and should be raised.
    pub(crate) fn take_pending_timer_irq(&mut self, cycle_count: u32) -> bool {
        let elapsed = cycle_count.wrapping_sub(self.last_irq_0_cycle_count);
        if elapsed >= self.cycles_per_irq {
            self.last_irq_0_cycle_count = cycle_count;
            log::trace!(
                "PIT: timer IRQ fired (cycle_count={cycle_count} cycles_per_irq={})",
                self.cycles_per_irq
            );
            true
        } else {
            false
        }
    }

    fn write_control(&mut self, val: u8, cycle_count: u32) {
        let channel = (val >> 6) & 0x03;
        let access = (val >> 4) & 0x03;
        match channel {
            0 => {
                if access == 0 {
                    // Counter latch command: capture current counter value.
                    let count = self.counter_value_ch0(cycle_count);
                    self.ch0.latched.set(Some(count));
                    self.ch0.latch_read_high.set(false);
                } else {
                    self.ch0.access_mode = access;
                    self.ch0.expect_low = access == 3;
                }
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

    fn memory_read_u8(&self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16, _cycle_count: u32) -> Option<u8> {
        match port {
            PIT_CHANNEL_0 => {
                if let Some(count) = self.ch0.latched.get() {
                    let high = self.ch0.latch_read_high.get();
                    let byte = if high {
                        (count >> 8) as u8
                    } else {
                        count as u8
                    };
                    self.ch0.latch_read_high.set(!high);
                    if high {
                        self.ch0.latched.set(None);
                        self.ch0.latch_read_high.set(false);
                    }
                    Some(byte)
                } else {
                    Some(0xFF)
                }
            }
            PIT_CHANNEL_1 | PIT_CHANNEL_2 | PIT_CONTROL => Some(0xFF),
            PORT_B => {
                // Bit 5: timer 2 output (high if channel 2 running and gate open).
                let timer2_out = if self.port_b & 0x01 != 0 { 0x20 } else { 0x00 };
                Some((self.port_b & 0x0F) | timer2_out)
            }
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8, cycle_count: u32) -> bool {
        match port {
            PIT_CONTROL => {
                self.write_control(val, cycle_count);
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
                    log::debug!(
                        "PIT: ch2 divisor={divisor} freq={:.1}Hz",
                        PIT_FREQUENCY_HZ as f32
                            / if divisor == 0 {
                                PIT_DIVISOR
                            } else {
                                divisor as u64
                            } as f32
                    );
                    self.ch2.divisor = divisor;
                    self.update_speaker();
                }
                true
            }
            PORT_B => {
                log::debug!(
                    "PIT: port B write 0x{val:02X} (gate={} speaker={})",
                    val & 1,
                    (val >> 1) & 1
                );
                self.port_b = val;
                self.update_speaker();
                true
            }
            _ => false,
        }
    }
}
