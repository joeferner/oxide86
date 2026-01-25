use anyhow::Result;

use crate::{cpu::Cpu, memory::Memory};

pub mod cpu;
pub mod memory;

pub struct Computer {
    cpu: Cpu,
    memory: Memory,
}

impl Computer {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            memory: Memory::new(),
        }
    }

    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<()> {
        self.memory.load_bios(bios_data)?;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
    }

    pub fn run(&mut self) {
        todo!()
    }
}
