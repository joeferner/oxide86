use std::any::Any;

use crate::{
    Device,
    disk::{Disk, DriveNumber},
};

pub struct HardDiskController {
    disks: Vec<Box<dyn Disk>>,
}

impl HardDiskController {
    pub fn new(disks: Vec<Box<dyn Disk>>) -> Self {
        Self { disks }
    }

    pub fn get_disk(&self, drive: DriveNumber) -> Option<&dyn Disk> {
        self.disks.get(drive.to_hard_drive_index()).map(|d| d.as_ref())
    }
}

impl Device for HardDiskController {
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

    fn io_read_u8(&self, _port: u16) -> Option<u8> {
        None
    }

    fn io_write_u8(&mut self, _port: u16, _val: u8) -> bool {
        false
    }
}
