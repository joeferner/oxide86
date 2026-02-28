use crate::{
    Devices,
    disk::{DiskError, DriveNumber},
};

pub struct IoBus {
    devices: Devices,
}

impl IoBus {
    pub fn new(devices: Devices) -> Self {
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

    pub fn disk_read_sectors(
        &self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, DiskError> {
        if let Some(disk_controller) = self.devices.find_disk_controller(drive) {
            disk_controller
                .borrow()
                .read_sectors(cylinder, head, sector, count)
        } else {
            Err(DiskError::DriveNotReady)
        }
    }
}
