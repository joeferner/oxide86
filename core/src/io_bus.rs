use std::{cell::RefCell, rc::Rc};

use crate::Device;

pub struct IoBus {
    devices: Rc<RefCell<Vec<Box<dyn Device>>>>,
}

impl IoBus {
    pub fn new(devices: Rc<RefCell<Vec<Box<dyn Device>>>>) -> Self {
        Self { devices }
    }

    pub fn write_u8(&mut self, addr: u16, val: u8) {
        let mut devices = self.devices.borrow_mut();
        for device in devices.iter_mut() {
            if device.io_write_u8(addr, val) {
                return;
            }
        }

        log::warn!("No device responded to io write addr: 0x{addr:04X}, val: 0x{val:02X}");
    }
}
