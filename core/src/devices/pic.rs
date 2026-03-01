use std::any::Any;

use crate::Device;

pub struct PIC {}

impl PIC {
    pub fn new() -> Self {
        Self {}
    }

    pub fn take_irq(&mut self) -> Option<u8> {
        todo!("take_irq");
    }
}

impl Device for PIC {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn memory_read_u8(&self, _addr: usize) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn io_read_u8(&self, _port: u16) -> Option<u8> {
        None
    }

    fn io_write_u8(&mut self, _port: u16, _val: u8) -> bool {
        false
    }
}
