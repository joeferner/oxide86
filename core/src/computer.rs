use crate::{cpu::Cpu, memory_bus::MemoryBus, physical_address};
use anyhow::Result;

pub struct Computer {
    memory_bus: MemoryBus,
    cpu: Cpu,
}

impl Computer {
    pub fn new(cpu: Cpu, memory_bus: MemoryBus) -> Self {
        Self { memory_bus, cpu }
    }

    /// Load a program at the specified segment:offset and set CPU to start there
    pub fn load_program(&mut self, program_data: &[u8], segment: u16, offset: u16) -> Result<()> {
        let physical_addr = physical_address(segment, offset);
        self.memory_bus.load_at(physical_addr, program_data)?;
        self.cpu.reset(segment, offset);
        Ok(())
    }

    pub fn run(&mut self) {
        while !self.cpu.is_halted() {
            self.step();
        }
    }

    pub fn step(&mut self) {
        self.cpu.step(&mut self.memory_bus);
    }

    pub fn is_halted(&self) -> bool {
        self.cpu.is_halted()
    }
}
