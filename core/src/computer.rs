use crate::{cpu::Cpu, io_bus::IoBus, memory_bus::MemoryBus, physical_address};
use anyhow::Result;

pub struct Computer {
    cpu: Cpu,
    memory_bus: MemoryBus,
    io_bus: IoBus,
}

impl Computer {
    pub fn new(cpu: Cpu, memory_bus: MemoryBus, io_bus: IoBus) -> Self {
        let mut computer = Self {
            cpu,
            memory_bus,
            io_bus,
        };
        computer.reset();
        computer
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
        self.cpu.step(&mut self.memory_bus, &mut self.io_bus);
    }

    pub fn is_halted(&self) -> bool {
        self.cpu.is_halted()
    }

    fn reset(&mut self) {
        self.memory_bus.reset();
        self.cpu.reset(0xffff, 0x0000);
    }
}
