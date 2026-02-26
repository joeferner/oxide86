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

    /// Load binary data at a specific address
    pub fn load_at(&mut self, address: usize, data: &[u8]) -> Result<()> {
        self.memory.load_at(address, data)
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
}
