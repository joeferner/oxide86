use crate::{cpu::Cpu, memory_bus::MemoryBus, physical_address};
use anyhow::Result;

pub struct Computer {
    memory_bus: MemoryBus,
    cpu: Cpu,
}

impl Computer {
    #[cfg(test)]
    pub fn new_for_test() -> Result<Computer> {
        use std::cell::RefCell;

        use crate::{memory::Memory, video::VideoCard};

        let video_card = RefCell::new(VideoCard::new());
        Ok(Computer {
            memory_bus: MemoryBus::new(Memory::new(2048 * 1024), video_card),
            cpu: Cpu::new(),
        })
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

    fn step(&mut self) {
        self.cpu.step(&mut self.memory_bus);
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
