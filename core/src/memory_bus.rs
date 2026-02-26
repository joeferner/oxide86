use std::cell::RefCell;

use anyhow::Result;

use crate::{
    memory::Memory,
    video::{CGA_MEMORY_END, CGA_MEMORY_START, VideoCard},
};

pub struct MemoryBus {
    memory: Memory,
    video: RefCell<VideoCard>,
}

impl MemoryBus {
    pub fn new(memory: Memory, video: RefCell<VideoCard>) -> Self {
        Self { memory, video }
    }

    pub fn read_u8(&self, addr: usize) -> u8 {
        // Route to Video for memory-mapped ranges
        if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&addr) {
            let offset = addr - CGA_MEMORY_START;
            return self.video.borrow().read_u8(offset);
        }

        self.memory.read_u8(addr)
    }

    pub fn write_u8(&mut self, addr: usize, val: u8) {
        // Route to Video for memory-mapped ranges
        if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&addr) {
            let offset = addr - CGA_MEMORY_START;
            self.video.borrow_mut().write_u8(offset, val);
            return;
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
