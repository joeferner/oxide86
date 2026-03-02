use std::any::Any;

use crate::Device;

pub const PIT_CHANNEL_0: u16 = 0x0040;
pub const PIT_CHANNEL_1: u16 = 0x0041;
pub const PIT_CHANNEL_2: u16 = 0x0042;
pub const PIT_CONTROL: u16 = 0x0043;

pub struct PIT {
    // PIT default: 1,193,182 Hz input clock ÷ 65,536 divisor ≈ 18.2 Hz
    cycles_per_irq: u32,
    last_irq_0_cycle_count: u32,
}

impl PIT {
    pub fn new(cpu_clock_speed: u32) -> Self {
        Self {
            cycles_per_irq: ((cpu_clock_speed as u64 * 65536) / 1_193_182) as u32,
            last_irq_0_cycle_count: 0,
        }
    }

    /// Returns `true` if a timer IRQ (IRQ 0) is pending and should be raised.
    ///
    /// The 8253/8254 PIT channel 0 fires at its default rate of approximately
    /// 18.2 Hz (1,193,182 Hz ÷ 65,536 default divisor). This method computes
    /// how many CPU cycles should elapse between interrupts based on
    /// `cpu_clock_speed`, then checks whether enough cycles have passed since
    /// `last_irq_0_cycle_count`.
    ///
    /// `cycle_count` is the running total of CPU cycles, wrapping on u32
    /// overflow. Wrapping subtraction is used so the counter rolls over safely.
    pub fn take_pending_timer_irq(&mut self, cycle_count: u32) -> bool {
        let elapsed = cycle_count.wrapping_sub(self.last_irq_0_cycle_count);

        if elapsed >= self.cycles_per_irq {
            self.last_irq_0_cycle_count = cycle_count;
            true
        } else {
            false
        }
    }
}

impl Device for PIT {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.last_irq_0_cycle_count = 0;
    }

    fn memory_read_u8(&self, _addr: usize) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16) -> Option<u8> {
        match port {
            PIT_CHANNEL_0 => None,
            PIT_CHANNEL_1 => None,
            PIT_CHANNEL_2 => None,
            PIT_CONTROL => None,
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, _val: u8) -> bool {
        match port {
            PIT_CHANNEL_0 => false,
            PIT_CHANNEL_1 => false,
            PIT_CHANNEL_2 => false,
            PIT_CONTROL => false,
            _ => false,
        }
    }
}
