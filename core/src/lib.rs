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

    /// Load a program at the specified segment:offset and set CPU to start there
    pub fn load_program(&mut self, program_data: &[u8], segment: u16, offset: u16) -> Result<()> {
        let physical_addr = Cpu::physical_address(segment, offset);
        self.memory.load_at(physical_addr, program_data)?;

        // Set CPU to start at this location
        self.cpu.cs = segment;
        self.cpu.ip = offset;

        // Initialize other segments to reasonable defaults
        self.cpu.ds = segment;
        self.cpu.es = segment;
        self.cpu.ss = segment;
        self.cpu.sp = 0xFFFE; // Stack grows down from top of segment

        Ok(())
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
    }

    pub fn run(&mut self) {
        self.cpu.run(&mut self.memory);
    }

    pub fn dump_registers(&self) {
        self.cpu.dump_registers();
    }
}
