use anyhow::Result;

use crate::{cpu::Cpu, memory::Memory};
pub use crate::cpu::bios::{Bios, NullBios};

pub mod cpu;
pub mod memory;

pub struct Computer<T: Bios = NullBios> {
    cpu: Cpu,
    memory: Memory,
    bios: T,
}

impl<T: Bios> Computer<T> {
    pub fn new(bios: T) -> Self {
        let mut memory = Memory::new();
        memory.initialize_ivt();
        Self {
            cpu: Cpu::new(),
            memory,
            bios,
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
        while !self.cpu.is_halted() {
            self.step();
        }
    }

    /// Execute a single instruction
    pub fn step(&mut self) {
        // Get current IP to check what opcode we're about to execute
        let current_ip = self.cpu.ip;
        let current_cs = self.cpu.cs;
        let addr = Cpu::physical_address(current_cs, current_ip);
        let opcode = self.memory.read_byte(addr);

        // Check if it's an INT instruction
        match opcode {
            0xCD => {
                // INT with immediate - need to fetch the interrupt number
                let int_num = self.memory.read_byte(addr + 1);
                // Manually advance IP past the INT instruction
                self.cpu.ip = self.cpu.ip.wrapping_add(2);
                // Execute with BIOS I/O
                self.cpu.execute_int_with_io(int_num, &mut self.memory, &mut self.bios);
            }
            0xCC => {
                // INT 3 - advance IP and execute INT 3
                self.cpu.ip = self.cpu.ip.wrapping_add(1);
                self.cpu.execute_int_with_io(3, &mut self.memory, &mut self.bios);
            }
            _ => {
                // Normal instruction - just use the regular step
                let opcode = self.cpu.fetch_byte(&self.memory);
                self.cpu.execute(opcode, &mut self.memory);
            }
        }
    }

    pub fn dump_registers(&self) {
        self.cpu.dump_registers();
    }
}
