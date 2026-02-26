use crate::{
    cpu::{Cpu, cpu_flag},
    memory::Memory,
    physical_address,
};
use anyhow::Result;

pub struct Computer {
    memory: Memory,
    cpu: Cpu,
}

impl Computer {
    #[cfg(test)]
    pub fn new_for_test() -> Result<Computer> {
        Ok(Computer {
            memory: Memory::new(2048 * 1024),
            cpu: Cpu::new(),
        })
    }

    /// Load a program at the specified segment:offset and set CPU to start there
    pub fn load_program(&mut self, program_data: &[u8], segment: u16, offset: u16) -> Result<()> {
        let physical_addr = physical_address(segment, offset);
        self.memory.load_at(physical_addr, program_data)?;

        // Set CPU to start at this location
        self.cpu.cs = segment;
        self.cpu.ip = offset;

        // Initialize other segments to reasonable defaults
        self.cpu.ds = segment;
        self.cpu.es = segment;
        self.cpu.ss = segment;
        self.cpu.sp = 0xFFFE; // Stack grows down from top of segment

        // Enable interrupts - DOS programs expect IF=1 (inherited from DOS environment)
        self.cpu.set_flag(cpu_flag::INTERRUPT, true);

        Ok(())
    }

    pub fn run(&mut self) {
        while !self.cpu.is_halted() {
            self.step();
        }
    }

    fn step(&self) {
        let addr = physical_address(self.cpu.cs, self.cpu.ip);
        let opcode = self.memory.read_u8(addr);

        todo!("unhandled opcode 0x{opcode:02X}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SEGMENT: u16 = 0x1000;
    const TEST_OFFSET: u16 = 0x0100;

    #[test]
    pub fn hello_world_video_memory() {
        let program_data = include_bytes!("test_data/hello_world_video_memory.com");
        let mut computer = Computer::new_for_test().unwrap();
        computer
            .load_program(program_data, TEST_SEGMENT, TEST_OFFSET)
            .unwrap();
        computer.run();
    }
}
