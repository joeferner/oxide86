use std::{cell::RefCell, rc::Rc};

use anyhow::Result;

use crate::{Device, cpu::bios::bios_reset, memory::Memory};

pub struct MemoryBus {
    memory: Memory,
    devices: Vec<Rc<RefCell<dyn Device>>>,
}

impl MemoryBus {
    pub fn new(memory: Memory, devices: Vec<Rc<RefCell<dyn Device>>>) -> Self {
        Self { memory, devices }
    }

    pub fn read_u8(&self, addr: usize) -> u8 {
        for device in &self.devices {
            if let Some(val) = device.borrow().memory_read_u8(addr) {
                return val;
            }
        }

        self.memory.read_u8(addr)
    }

    pub fn write_u8(&mut self, addr: usize, val: u8) {
        for device in &self.devices {
            if device.borrow_mut().memory_write_u8(addr, val) {
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

    /// Read 32-bit dword from memory or memory-mapped device
    pub fn read_u32(&self, address: usize) -> u32 {
        let w1 = self.read_u16(address) as u32;
        let w2 = self.read_u16(address + 2) as u32;
        (w2 << 16) | w1
    }

    /// Write 32-bit dword to memory or memory-mapped device
    pub fn write_u32(&mut self, address: usize, value: u32) {
        self.write_u16(address, (value & 0xFFFF) as u16);
        self.write_u16(address + 2, (value >> 16) as u16);
    }

    /// Load binary data at a specific address
    pub fn load_at(&mut self, addr: usize, data: &[u8]) -> Result<()> {
        self.memory.load_at(addr, data)
    }

    pub fn reset(&mut self) {
        bios_reset(self);
    }
}
