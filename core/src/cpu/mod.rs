use crate::{cpu::bios::BIOS_CODE_SEGMENT, io_bus::IoBus, memory_bus::MemoryBus, physical_address};
pub mod bios;
mod cpu_type;
mod instructions;
mod timing;

pub use cpu_type::CpuType;

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

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum RepeatPrefix {
    Rep,   // 0xF3 - Repeat while CX != 0
    Repe,  // 0xF3 - Repeat while CX != 0 and ZF = 1
    Repne, // 0xF2 - Repeat while CX != 0 and ZF = 0
}

pub struct Cpu {
    cpu_type: CpuType,

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

    /// Repeat prefix for string instructions
    repeat_prefix: Option<RepeatPrefix>,

    /// Pending CPU exception interrupt number (e.g. 0 = divide error)
    /// Set by instructions that trigger CPU exceptions; fired by Computer::step()
    pending_exception: Option<u8>,
}

impl Cpu {
    pub fn new(cpu_type: CpuType) -> Self {
        Self {
            cpu_type,
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
            repeat_prefix: None,
            pending_exception: None,
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
            // OR immediate to AL/AX
            0x0C..=0x0D => self.or_imm_acc(opcode, memory_bus),

            // AND immediate to AL/AX
            0x24..=0x25 => self.and_imm_acc(opcode, memory_bus),

            // ES: segment override prefix (26)
            0x26 => {
                self.segment_override = Some(self.es);
                self.step(memory_bus, io_bus);
                self.segment_override = None;
            }

            // CMP r/m to register
            0x38..=0x3B => self.cmp_rm_reg(opcode, memory_bus),

            // CMP immediate to AL/AX
            0x3C..=0x3D => self.cmp_imm_acc(opcode, memory_bus),

            // INC 16-bit register (40-47)
            0x40..=0x47 => self.inc_reg16(opcode),

            // DEC 16-bit register (48-4F)
            0x48..=0x4F => self.dec_reg16(opcode),

            // PUSH 16-bit register (50-57)
            0x50..=0x57 => self.push_reg16(opcode, memory_bus),

            // POP 16-bit register (58-5F)
            0x58..=0x5F => self.pop_reg16(opcode, memory_bus),

            // Conditional jumps (70-7F)
            0x70..=0x7F => self.jmp_conditional(opcode, memory_bus),

            // Arithmetic/logical immediate to r/m (80: 8-bit, 81: 16-bit, 82: same as 80, 83: sign-extended 8-bit to 16-bit)
            0x80 | 0x82 => self.arith_imm8_rm8(memory_bus),
            0x81 => self.arith_imm16_rm(memory_bus),
            0x83 => self.arith_imm8_rm(memory_bus),

            // TEST r/m and register (84-85)
            0x84..=0x85 => self.test_rm_reg(opcode, memory_bus),

            // MOV register to/from r/m (88-8B)
            0x88..=0x8B => self.mov_reg_rm(opcode, memory_bus),

            // MOV r/m16 to segment register (8E)
            0x8E => self.mov_rm_to_segreg(memory_bus),

            // MOV accumulator (AL/AX) to/from direct memory offset (A0-A3)
            0xA0..=0xA3 => self.mov_acc_moffs(opcode, memory_bus),

            // TEST immediate to AL/AX (A8-A9)
            0xA8..=0xA9 => self.test_imm_acc(opcode, memory_bus),

            // LODS - Load String (AC-AD)
            0xAC..=0xAD => self.lods(opcode, memory_bus),

            // MOV immediate to register (B0-BF)
            0xB0..=0xBF => self.mov_imm_to_reg(opcode, memory_bus),

            // RET with optional pop (C2: with imm16, C3: without)
            0xC2..=0xC3 => self.ret(opcode, memory_bus),

            // MOV immediate to r/m (C6: 8-bit, C7: 16-bit)
            0xC6..=0xC7 => self.mov_imm_to_rm(opcode, memory_bus),

            // INT - Software Interrupt (CD)
            0xCD => self.int(memory_bus),

            // Shift/Rotate Group 2 (D0: r/m8, 1; D1: r/m16, 1; D2: r/m8, CL; D3: r/m16, CL)
            0xD0..=0xD3 => self.shift_rotate_group(opcode, memory_bus),

            // CALL near relative (E8)
            0xE8 => self.call_near(memory_bus),

            // JMP short relative (EB)
            0xEB => self.jmp_short(memory_bus),

            // HLT - Halt (F4)
            0xF4 => self.hlt(),

            // NOT/NEG/MUL/DIV Group 3 (F6: 8-bit, F7: 16-bit)
            0xF6..=0xF7 => self.unary_group3(opcode, memory_bus),

            // INC/DEC/CALL/JMP Group 4/5 (FE: 8-bit, FF: 16-bit)
            0xFE => self.inc_dec_rm(opcode, memory_bus),
            0xFF => {
                // For FF, we need to check the reg field to determine operation
                let modrm_peek = memory_bus.read_u8(physical_address(self.cs, self.ip));
                let reg_field = (modrm_peek >> 3) & 0x07;
                match reg_field {
                    0 | 1 => self.inc_dec_rm(opcode, memory_bus), // INC/DEC
                    2 | 3 => self.call_indirect(memory_bus),      // CALL near/far
                    4 | 5 => self.jmp_indirect(memory_bus),       // JMP near/far
                    6 => self.push_rm16(memory_bus),              // PUSH r/m16
                    _ => log::warn!(
                        "Invalid FF /{}  at {:04X}:{:04X} (undefined, skipping)",
                        reg_field,
                        self.cs,
                        self.ip.wrapping_sub(1)
                    ),
                }
            }

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
        self.flags = 0x0002; // Reserved bit always set
        self.halted = false;
        self.last_instruction_cycles = 0;
        self.segment_override = None;
        self.repeat_prefix = None;
        self.pending_exception = None;

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
    use std::sync::Arc;
    use std::{cell::RefCell, rc::Rc};

    use crate::DeviceRef;
    use crate::cpu::CpuType;
    use crate::{
        cpu::Cpu,
        memory::Memory,
        memory_bus::MemoryBus,
        video::{VideoBuffer, VideoCard},
    };

    pub fn create_test_cpu() -> (Cpu, MemoryBus) {
        let cpu = Cpu::new(CpuType::I8086);
        let video_buffer = Arc::new(VideoBuffer::new());
        let devices: Vec<DeviceRef> = vec![Rc::new(RefCell::new(VideoCard::new(video_buffer)))];
        let memory_bus = MemoryBus::new(Memory::new(1024), devices);

        (cpu, memory_bus)
    }
}
