use std::cell::RefCell;

use anyhow::Result;

use crate::{Device, memory::Memory};

pub struct MemoryBus {
    memory: Memory,
    devices: Vec<RefCell<Box<dyn Device>>>,
}

impl MemoryBus {
    pub fn new(memory: Memory, devices: Vec<RefCell<Box<dyn Device>>>) -> Self {
        Self { memory, devices }
    }

    pub fn read_u8(&self, addr: usize) -> u8 {
        for device in &self.devices {
            if let Some(val) = device.borrow().read_u8(addr) {
                return val;
            }
        }

        self.memory.read_u8(addr)
    }

    pub fn write_u8(&mut self, addr: usize, val: u8) {
        for device in &self.devices {
            if device.borrow_mut().write_u8(addr, val) {
                return;
            }
        }

        self.memory.write_u8(addr, val);
    }

    /// Read a 16-bit word (little-endian)
    pub fn read_u16(&self, address: usize) -> u16 {
        let low = self.read_u8(address) as u16;
        let high = self.read_u8(address + 1) as u16;
        (high << 8) | low
    }

    /// Write a 16-bit word (little-endian)
    pub fn write_u16(&mut self, addr: usize, val: u16) {
        self.write_u8(addr, (val & 0xFF) as u8);
        self.write_u8(addr + 1, (val >> 8) as u8);
    }

    /// Load binary data at a specific address
    pub fn load_at(&mut self, addr: usize, data: &[u8]) -> Result<()> {
        self.memory.load_at(addr, data)
    }
}
