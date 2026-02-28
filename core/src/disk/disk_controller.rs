use std::any::Any;

use crate::{
    Device,
    disk::{Disk, DiskError, DriveNumber},
};

pub struct DiskController {
    drive_number: DriveNumber,
    disk: Box<dyn Disk>,
}

impl DiskController {
    pub fn new(drive_number: DriveNumber, disk: Box<dyn Disk>) -> Self {
        Self { drive_number, disk }
    }

    pub fn drive_number(&self) -> DriveNumber {
        self.drive_number
    }

    pub fn read_sectors(
        &self,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, DiskError> {
        self.disk.read_sectors(cylinder, head, sector, count)
    }
}

impl Device for DiskController {
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
