use crate::{
    Devices,
    cpu::bios::{
        int13_disk_services::DriveParams,
        int17_printer_services::{PrinterStatus, printer_status},
    },
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

    pub fn disk_get_params(&self, drive: DriveNumber) -> Result<DriveParams, DiskError> {
        let disk_controller = self
            .devices
            .find_disk_controller(drive)
            .ok_or(DiskError::DriveNotReady)?;

        let geometry = disk_controller.borrow().disk_geometry();

        // Count drives of this type (CD-ROM placeholders excluded from hard drive count)
        // TODO
        // let drive_count = if drive.is_floppy() {
        //     self.devices.floppy_drives_with_disk_count()
        // } else {
        //     self.devices.hard_drive_count()
        // };
        let drive_count = 0;

        Ok(DriveParams {
            max_cylinder: (geometry.cylinders - 1).min(255) as u8,
            max_head: (geometry.heads - 1).min(255) as u8,
            max_sector: geometry.sectors_per_track.min(255) as u8,
            drive_count,
        })
    }

    pub fn printer_init(&self, _printer: u8) -> PrinterStatus {
        // No printer available - return timeout status
        PrinterStatus {
            status: printer_status::TIMEOUT,
        }
    }
}
