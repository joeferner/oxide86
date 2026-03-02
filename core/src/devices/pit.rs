use std::any::Any;

use crate::Device;

pub const PIT_CHANNEL_0: u16 = 0x0040;
pub const PIT_CHANNEL_1: u16 = 0x0041;
pub const PIT_CHANNEL_2: u16 = 0x0042;
pub const PIT_CONTROL: u16 = 0x0043;

pub struct PIT {}

impl PIT {
    pub fn new() -> Self {
        Self {}
    }

    pub fn take_pending_timer_irq(&mut self, cycle_count: u32) -> bool {
        todo!("calculate if timer should fire {cycle_count}");
    }
}

impl Device for PIT {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {}

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
