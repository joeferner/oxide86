use crate::memory::Memory;

mod instructions;

pub struct Cpu {
    // General purpose registers
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,

    // Index and pointer registers
    pub si: u16,
    pub di: u16,
    pub sp: u16,
    pub bp: u16,

    // Segment registers
    pub cs: u16,
    pub ds: u16,
    pub ss: u16,
    pub es: u16,

    // Instruction pointer
    pub ip: u16,

    // Flags (start with just carry, zero, sign)
    pub flags: u16,

    // Halted flag
    halted: bool,
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
            ip: 0,
            flags: 0,
            halted: false,
        }
    }

    // Reset CPU to initial state (as if powered on)
    pub fn reset(&mut self) {
        // On x86 reset, CS:IP = 0xF000:0xFFF0 (physical address 0xFFFF0)
        self.cs = 0xF000;
        self.ip = 0xFFF0;

        // Other typical reset values
        self.flags = 0x0002; // Reserved bit always set
        self.sp = 0;
        self.halted = false;
        // Other registers are undefined on reset
    }

    // Calculate physical address from segment:offset
    pub fn physical_address(segment: u16, offset: u16) -> usize {
        ((segment as usize) << 4) + (offset as usize)
    }

    // Fetch a byte from memory at CS:IP and increment IP
    pub(super) fn fetch_byte(&mut self, memory: &Memory) -> u8 {
        let addr = Self::physical_address(self.cs, self.ip);
        let byte = memory.read_byte(addr);
        self.ip = self.ip.wrapping_add(1);
        byte
    }

    // Fetch a word (2 bytes, little-endian) from memory at CS:IP
    pub(super) fn fetch_word(&mut self, memory: &Memory) -> u16 {
        let low = self.fetch_byte(memory) as u16;
        let high = self.fetch_byte(memory) as u16;
        (high << 8) | low
    }

    // Main execution loop
    pub fn run(&mut self, memory: &mut Memory) {
        while !self.halted {
            self.step(memory);
        }
    }

    // Execute a single instruction
    fn step(&mut self, memory: &mut Memory) {
        let opcode = self.fetch_byte(memory);
        self.execute(opcode, memory);
    }

    // Decode and execute instruction
    fn execute(&mut self, opcode: u8, memory: &mut Memory) {
        match opcode {
            // HLT - Halt
            0xF4 => {
                self.halted = true;
            }

            // MOV immediate to register (B0-BF)
            0xB0..=0xBF => self.mov_imm_to_reg(opcode, memory),

            _ => {
                panic!("Unknown opcode: {:#04X} at {:04X}:{:04X}", opcode, self.cs, self.ip.wrapping_sub(1));
            }
        }
    }

    // Set 8-bit register
    pub(super) fn set_reg8(&mut self, reg: u8, value: u8) {
        match reg {
            0 => self.ax = (self.ax & 0xFF00) | value as u16,        // AL
            1 => self.cx = (self.cx & 0xFF00) | value as u16,        // CL
            2 => self.dx = (self.dx & 0xFF00) | value as u16,        // DL
            3 => self.bx = (self.bx & 0xFF00) | value as u16,        // BL
            4 => self.ax = (self.ax & 0x00FF) | ((value as u16) << 8), // AH
            5 => self.cx = (self.cx & 0x00FF) | ((value as u16) << 8), // CH
            6 => self.dx = (self.dx & 0x00FF) | ((value as u16) << 8), // DH
            7 => self.bx = (self.bx & 0x00FF) | ((value as u16) << 8), // BH
            _ => unreachable!(),
        }
    }

    // Set 16-bit register
    pub(super) fn set_reg16(&mut self, reg: u8, value: u16) {
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

    // Dump register state
    pub fn dump_registers(&self) {
        println!("AX={:04X}  BX={:04X}  CX={:04X}  DX={:04X}", self.ax, self.bx, self.cx, self.dx);
        println!("SI={:04X}  DI={:04X}  BP={:04X}  SP={:04X}", self.si, self.di, self.bp, self.sp);
        println!("CS={:04X}  DS={:04X}  SS={:04X}  ES={:04X}", self.cs, self.ds, self.ss, self.es);
        println!("IP={:04X}  FLAGS={:04X}", self.ip, self.flags);
    }
}
