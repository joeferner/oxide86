use crate::DeviceRef;

pub struct IoBus {
    devices: Vec<DeviceRef>,
}

impl IoBus {
    pub fn new(devices: Vec<DeviceRef>) -> Self {
        Self { devices }
    }

    pub fn read_u8(&self, port: u16) -> u8 {
        for device in &self.devices {
            if let Some(val) = device.borrow().io_read_u8(port) {
                return val;
            }
        }

        log::warn!("No device responded to io write port: 0x{port:04X}");
        0xff
    }

    pub fn read_u16(&self, port: u16) -> u16 {
        todo!("IoBus read_u16 {port}");
    }

    pub fn write_u8(&mut self, port: u16, val: u8) {
        for device in &self.devices {
            if device.borrow_mut().io_write_u8(port, val) {
                return;
            }
        }

        log::warn!("No device responded to io write port: 0x{port:04X}, val: 0x{val:02X}");
    }
}
