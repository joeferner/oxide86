use crate::DeviceRef;

pub struct IoBus {
    devices: Vec<DeviceRef>,
}

impl IoBus {
    pub fn new(devices: Vec<DeviceRef>) -> Self {
        Self { devices }
    }

    pub fn write_u8(&mut self, addr: u16, val: u8) {
        for device in &self.devices {
            if device.borrow_mut().io_write_u8(addr, val) {
                return;
            }
        }

        log::warn!("No device responded to io write addr: 0x{addr:04X}, val: 0x{val:02X}");
    }
}
