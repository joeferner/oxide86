use std::{any::Any, cell::RefCell, rc::Rc};

use anyhow::{Context, Result};

use crate::disk::{DiskController, DriveNumber};

pub mod computer;
pub mod cpu;
pub mod disk;
pub mod io_bus;
pub mod memory;
pub mod memory_bus;
pub mod video;

#[cfg(test)]
pub mod tests;

// Calculate physical address from segment:offset
pub fn physical_address(segment: u16, offset: u16) -> usize {
    ((segment as usize) << 4) + (offset as usize)
}

pub fn parse_hex_or_dec(s: &str) -> Result<u16> {
    if let Some(hex) = s.strip_prefix("0x") {
        u16::from_str_radix(hex, 16).with_context(|| format!("Invalid hex value: {}", s))
    } else {
        s.parse::<u16>()
            .with_context(|| format!("Invalid decimal value: {}", s))
    }
}

pub type DeviceRef = Rc<RefCell<dyn Device>>;

pub trait Device {
    fn as_any(&self) -> &dyn Any;

    fn memory_read_u8(&self, addr: usize) -> Option<u8>;
    fn memory_write_u8(&mut self, addr: usize, val: u8) -> bool;

    fn io_read_u8(&self, port: u16) -> Option<u8>;
    fn io_write_u8(&mut self, port: u16, val: u8) -> bool;
}

#[derive(Clone)]
pub struct Devices {
    list: Vec<DeviceRef>,
    disk_controllers: Vec<Rc<RefCell<DiskController>>>,
}

impl Devices {
    pub fn new() -> Self {
        Self {
            list: vec![],
            disk_controllers: vec![],
        }
    }

    pub fn push<T: Device + 'static>(&mut self, device: T) {
        let rc = Rc::new(RefCell::new(device));
        let rc_any: Rc<dyn Any> = rc.clone();
        if let Ok(dc) = Rc::downcast::<RefCell<DiskController>>(rc_any) {
            self.disk_controllers.push(dc);
        }
        self.list.push(rc);
    }

    pub fn find_disk_controller(&self, drive: DriveNumber) -> Option<Rc<RefCell<DiskController>>> {
        self.disk_controllers
            .iter()
            .find(|c| c.borrow().drive_number() == drive)
            .cloned()
    }
}

impl<'a> IntoIterator for &'a Devices {
    type Item = &'a DeviceRef;
    type IntoIter = std::slice::Iter<'a, DeviceRef>;

    fn into_iter(self) -> Self::IntoIter {
        self.list.iter()
    }
}
