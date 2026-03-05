use anyhow::{Result, anyhow};

pub struct Memory {
    data: Vec<u8>,
}

impl Memory {
    pub(crate) fn new(physical_size: usize) -> Self {
        Self {
            data: vec![0; physical_size],
        }
    }

    /// Load binary data at a specific address
    pub(crate) fn load_at(&mut self, address: usize, data: &[u8]) -> Result<()> {
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

    pub(crate) fn read_u8(&self, addr: usize) -> u8 {
        if addr >= self.data.len() {
            return 0xFF; // Reading beyond memory returns 0xFF
        }
        self.data[addr]
    }

    pub(crate) fn write_u8(&mut self, addr: usize, val: u8) {
        if addr >= self.data.len() {
            // Writing beyond memory is silently ignored
            return;
        }
        self.data[addr] = val;
    }

    /// Extended memory in KB above 1 MB (reported via INT 15h AH=88h on 286+)
    pub(crate) fn extended_memory_kb(&self) -> u16 {
        (self.data.len() / 1024)
            .saturating_sub(1024)
            .min(u16::MAX as usize) as u16
    }
}
