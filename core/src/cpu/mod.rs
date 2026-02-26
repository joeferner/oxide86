use crate::{memory::MemoryBus, physical_address};
mod instructions;
mod timing;

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

    // Instruction pointer
    ip: u16,

    // Flags (start with just carry, zero, sign)
    flags: u16,

    // Halted flag
    halted: bool,

    /// Cycle count for the last executed instruction
    /// Used by Computer::step() to accurately track CPU cycles
    last_instruction_cycles: u64,
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

    pub fn step(&mut self, memory_bus: &MemoryBus) {
        let opcode = self.fetch_byte(memory_bus);
        match opcode {
            // MOV immediate to register (B0-BF)
            0xB0..=0xBF => self.mov_imm_to_reg(opcode, memory_bus),

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

    /// Set 8-bit register
    fn set_reg8(&mut self, reg: u8, value: u8) {
        match reg {
            0 => self.ax = (self.ax & 0xFF00) | value as u16, // AL
            1 => self.cx = (self.cx & 0xFF00) | value as u16, // CL
            2 => self.dx = (self.dx & 0xFF00) | value as u16, // DL
            3 => self.bx = (self.bx & 0xFF00) | value as u16, // BL
            4 => self.ax = (self.ax & 0x00FF) | ((value as u16) << 8), // AH
            5 => self.cx = (self.cx & 0x00FF) | ((value as u16) << 8), // CH
            6 => self.dx = (self.dx & 0x00FF) | ((value as u16) << 8), // DH
            7 => self.bx = (self.bx & 0x00FF) | ((value as u16) << 8), // BH
            _ => unreachable!(),
        }
    }

    /// Set 16-bit register
    fn set_reg16(&mut self, reg: u8, value: u16) {
        match reg & 0x07 {
            0 => self.ax = value,
            1 => self.cx = value,
            2 => self.dx = value,
            3 => self.bx = value,
            4 => self.sp = value,
            5 => self.bp = value,
            6 => self.si = value,
            7 => self.di = value,
            _ => unreachable!(),
        }
    }

    pub fn reset(&mut self, segment: u16, offset: u16) {
        self.ax = 0;
        self.bx = 0;
        self.cx = 0;
        self.dx = 0;
        self.si = 0;
        self.di = 0;
        self.sp = 0;
        self.bp = 0;
        self.cs = 0;
        self.ds = 0;
        self.ss = 0;
        self.es = 0;
        self.fs = 0;
        self.gs = 0;
        self.ip = 0;
        self.flags = 0;
        self.halted = false;

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
}
