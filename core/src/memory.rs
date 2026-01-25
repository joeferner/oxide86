use anyhow::{Result, anyhow};

// 1MB = 0x100000 bytes
pub const MEMORY_SIZE: usize = 0x100000;

pub struct Memory {
    data: Vec<u8>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            data: vec![0; MEMORY_SIZE],
        }
    }

    // Load binary data at a specific address
    pub fn load_at(&mut self, address: usize, data: &[u8]) -> Result<()> {
        if address + data.len() > MEMORY_SIZE {
            return Err(anyhow!(
                "Data exceeds memory bounds: {address:#x} + {:#x} > {MEMORY_SIZE:#x}",
                data.len()
            ));
        }

        self.data[address..address + data.len()].copy_from_slice(data);
        Ok(())
    }

    // Load BIOS - typically at the end of the first megabyte
    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<()> {
        let bios_size = bios_data.len();

        // BIOS is loaded at the top of memory
        // For a 64KB BIOS: 0x100000 - 0x10000 = 0xF0000
        let bios_start = MEMORY_SIZE - bios_size;

        self.load_at(bios_start, bios_data)
    }

    pub fn read_byte(&self, address: usize) -> u8 {
        self.data[address % MEMORY_SIZE]
    }

    pub fn write_byte(&mut self, address: usize, value: u8) {
        self.data[address % MEMORY_SIZE] = value;
    }

    // Read a 16-bit word (little-endian)
    pub fn read_word(&self, address: usize) -> u16 {
        let low = self.read_byte(address) as u16;
        let high = self.read_byte(address + 1) as u16;
        (high << 8) | low
    }

    // Write a 16-bit word (little-endian)
    pub fn write_word(&mut self, address: usize, value: u16) {
        self.write_byte(address, (value & 0xFF) as u8);
        self.write_byte(address + 1, (value >> 8) as u8);
    }
}
