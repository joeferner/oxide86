use crate::{cpu::bios::BIOS_CODE_SEGMENT, io_bus::IoBus, memory_bus::MemoryBus, physical_address};
pub mod bios;
mod instructions;
mod timing;

// IVT (Interrupt Vector Table) constants
pub const IVT_START: usize = 0x0000;
pub const IVT_END: usize = 0x0400;
pub const IVT_ENTRY_SIZE: usize = 4; // Each entry is 4 bytes (offset, segment)

/// Flag bit positions
#[allow(dead_code)]
mod cpu_flag {
    pub const CARRY: u16 = 1 << 0;
    pub const PARITY: u16 = 1 << 2;
    pub const AUXILIARY: u16 = 1 << 4;
    pub const ZERO: u16 = 1 << 6;
    pub const SIGN: u16 = 1 << 7;
    pub const TRAP: u16 = 1 << 8;
    pub const INTERRUPT: u16 = 1 << 9;
    pub const DIRECTION: u16 = 1 << 10;
    pub const OVERFLOW: u16 = 1 << 11;
}

pub struct Cpu {
    // General purpose registers
    ax: u16,
    bx: u16,
    cx: u16,
    dx: u16,

    // Index and pointer registers
    si: u16,
    di: u16,
    sp: u16,
    bp: u16,

    // Segment registers
    cs: u16,
    ds: u16,
    ss: u16,
    es: u16,
    fs: u16, // 80386+
    gs: u16, // 80386+

    /// Instruction pointer
    ip: u16,

    /// Flags (start with just carry, zero, sign)
    flags: u16,

    /// Halted flag
    halted: bool,

    /// Cycle count for the last executed instruction
    /// Used by Computer::step() to accurately track CPU cycles
    last_instruction_cycles: u64,

    /// Segment override prefix (for next instruction only)
    segment_override: Option<u16>,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            ax: 0,
            bx: 0,
            cx: 0,
            dx: 0,
            si: 0,
            di: 0,
            sp: 0,
            bp: 0,
            cs: 0,
            ds: 0,
            ss: 0,
            es: 0,
            fs: 0,
            gs: 0,
            ip: 0,
            flags: 0,
            halted: false,
            last_instruction_cycles: 0,
            segment_override: None,
        }
    }

    /// Set a specific flag
    fn set_flag(&mut self, flag: u16, value: bool) {
        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    /// Get a specific flag
    #[allow(dead_code)]
    fn get_flag(&self, flag: u16) -> bool {
        (self.flags & flag) != 0
    }

    /// Check if CPU is halted
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    pub fn step(&mut self, memory_bus: &mut MemoryBus, io_bus: &mut IoBus) {
        if self.cs == BIOS_CODE_SEGMENT {
            self.step_bios_int(memory_bus, io_bus);
            return;
        }

        let opcode = self.fetch_byte(memory_bus);
        match opcode {
            // ES: segment override prefix (26)
            0x26 => {
                self.segment_override = Some(self.es);
                self.step(memory_bus, io_bus);
                self.segment_override = None;
            }

            // MOV r/m16 to segment register (8E)
            0x8E => self.mov_rm_to_segreg(memory_bus),

            // MOV accumulator (AL/AX) to/from direct memory offset (A0-A3)
            0xA0..=0xA3 => self.mov_acc_moffs(opcode, memory_bus),

            // MOV immediate to register (B0-BF)
            0xB0..=0xBF => self.mov_imm_to_reg(opcode, memory_bus),

            // INT - Software Interrupt (CD)
            0xCD => self.int(memory_bus),

            // HLT - Halt (F4)
            0xF4 => self.hlt(),

            _ => {
                let err = format!(
                    "Unknown opcode: {:#04X} at {:04X}:{:04X}",
                    opcode,
                    self.cs,
                    self.ip.wrapping_sub(1)
                );
                log::error!("{}", err);
                panic!("{}", err);
            }
        }
    }

    /// Fetch a byte from memory at CS:IP and increment IP
    fn fetch_byte(&mut self, memory_bus: &MemoryBus) -> u8 {
        let addr = physical_address(self.cs, self.ip);
        let byte = memory_bus.read_u8(addr);
        self.ip = self.ip.wrapping_add(1);
        byte
    }

    /// Fetch a word (2 bytes, little-endian) from memory at CS:IP
    fn fetch_word(&mut self, memory_bus: &MemoryBus) -> u16 {
        let low = self.fetch_byte(memory_bus) as u16;
        let high = self.fetch_byte(memory_bus) as u16;
        (high << 8) | low
    }

    pub fn reset(&mut self, segment: u16, offset: u16) {
        self.ax = 0;
        self.bx = 0;
        self.cx = 0;
        self.dx = 0;
        self.si = 0;
        self.di = 0;
        self.bp = 0;
        self.cs = 0;
        self.ds = 0;
        self.es = 0;
        self.fs = 0;
        self.gs = 0;
        self.ip = 0;
        self.flags = 0;
        self.halted = false;
        self.last_instruction_cycles = 0;
        self.segment_override = None;

        // Set CPU to start at this location
        self.cs = segment;
        self.ip = offset;

        // Initialize other segments to reasonable defaults
        self.ds = segment;
        self.es = segment;
        self.ss = segment;
        self.sp = 0xFFFE; // Stack grows down from top of segment

        // Enable interrupts - DOS programs expect IF=1 (inherited from DOS environment)
        self.set_flag(cpu_flag::INTERRUPT, true);
    }

    fn step_bios_int(&mut self, memory_bus: &mut MemoryBus, io_bus: &mut IoBus) {
        let int = self.ip / 4;
        match int {
            0x21 => self.handle_int21_dos_services(memory_bus, io_bus),
            _ => log::error!("unhandled BIOS interrupt 0x{int:04X}"),
        }
        self.iret(memory_bus);
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};
    use std::sync::Arc;

    use crate::{
        Device,
        cpu::Cpu,
        memory::Memory,
        memory_bus::MemoryBus,
        video::{VideoBuffer, VideoCard},
    };

    pub fn create_test_cpu() -> (Cpu, MemoryBus) {
        let cpu = Cpu::new();
        let video_buffer = Arc::new(VideoBuffer::new());
        let devices: Rc<RefCell<Vec<Box<dyn Device>>>> =
            Rc::new(RefCell::new(vec![Box::new(VideoCard::new(video_buffer))]));
        let memory_bus = MemoryBus::new(Memory::new(1024), devices);

        (cpu, memory_bus)
    }
}
