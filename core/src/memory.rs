use anyhow::{Result, anyhow};

pub struct MemoryBus {
    memory: Memory,
}

impl MemoryBus {
    pub fn new(memory: Memory) -> Self {
        Self { memory }
    }

    pub fn read_u8(&self, addr: usize) -> u8 {
        self.memory.read_u8(addr)
    }

    pub fn write_u8(&mut self, addr: usize, val: u8) {
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

pub struct Memory {
    data: Vec<u8>,
}

impl Memory {
    pub fn new(physical_size: usize) -> Self {
        Self {
            data: vec![0; physical_size],
        }
    }

    /// Load binary data at a specific address
    pub fn load_at(&mut self, address: usize, data: &[u8]) -> Result<()> {
        if address + data.len() > self.data.len() {
            return Err(anyhow!(
                "Data exceeds memory bounds: {address:#x} + {:#x} > {:#x}",
                data.len(),
                self.data.len()
            ));
        }

        self.data[address..address + data.len()].copy_from_slice(data);
        Ok(())
    }

    pub fn read_u8(&self, addr: usize) -> u8 {
        if addr >= self.data.len() {
            return 0xFF; // Reading beyond memory returns 0xFF
        }
        self.data[addr]
    }

    fn write_u8(&mut self, addr: usize, val: u8) {
        if addr >= self.data.len() {
            // Writing beyond memory is silently ignored
            return;
        }
        self.data[addr] = val;
    }
}
