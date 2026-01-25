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

// Flag bit positions
const FLAG_CARRY: u16 = 1 << 0;
const FLAG_PARITY: u16 = 1 << 2;
const FLAG_AUXILIARY: u16 = 1 << 4;
const FLAG_ZERO: u16 = 1 << 6;
const FLAG_SIGN: u16 = 1 << 7;
const FLAG_TRAP: u16 = 1 << 8;
const FLAG_INTERRUPT: u16 = 1 << 9;
const FLAG_DIRECTION: u16 = 1 << 10;
const FLAG_OVERFLOW: u16 = 1 << 11;

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
            // ADD r/m to register
            0x00..=0x03 => self.add_rm_reg(opcode, memory),

            // ADD immediate to AL/AX
            0x04..=0x05 => self.add_imm_acc(opcode, memory),

            // OR r/m to register
            0x08..=0x0B => self.or_rm_reg(opcode, memory),

            // OR immediate to AL/AX
            0x0C..=0x0D => self.or_imm_acc(opcode, memory),

            // AND r/m to register
            0x20..=0x23 => self.and_rm_reg(opcode, memory),

            // AND immediate to AL/AX
            0x24..=0x25 => self.and_imm_acc(opcode, memory),

            // SUB r/m to register
            0x28..=0x2B => self.sub_rm_reg(opcode, memory),

            // SUB immediate to AL/AX
            0x2C..=0x2D => self.sub_imm_acc(opcode, memory),

            // XOR r/m to register
            0x30..=0x33 => self.xor_rm_reg(opcode, memory),

            // XOR immediate to AL/AX
            0x34..=0x35 => self.xor_imm_acc(opcode, memory),

            // CMP r/m to register
            0x38..=0x3B => self.cmp_rm_reg(opcode, memory),

            // CMP immediate to AL/AX
            0x3C..=0x3D => self.cmp_imm_acc(opcode, memory),

            // INC 16-bit register (40-47)
            0x40..=0x47 => self.inc_reg16(opcode),

            // DEC 16-bit register (48-4F)
            0x48..=0x4F => self.dec_reg16(opcode),

            // PUSH 16-bit register (50-57)
            0x50..=0x57 => self.push_reg16(opcode, memory),

            // POP 16-bit register (58-5F)
            0x58..=0x5F => self.pop_reg16(opcode, memory),

            // PUSH immediate (68: imm16, 6A: imm8 sign-extended)
            0x68 | 0x6A => self.push_imm(opcode, memory),

            // Conditional jumps (70-7F)
            0x70..=0x7F => self.jmp_conditional(opcode, memory),

            // Arithmetic/logical immediate to r/m (80: 8-bit, 81: 16-bit, 83: sign-extended 8-bit to 16-bit)
            0x80 => self.arith_imm8_rm8(memory),
            0x81 => self.arith_imm16_rm(memory),
            0x83 => self.arith_imm8_rm(memory),

            // TEST r/m and register (84-85)
            0x84..=0x85 => self.test_rm_reg(opcode, memory),

            // MOV register to/from r/m (88-8B)
            0x88..=0x8B => self.mov_reg_rm(opcode, memory),

            // MOV accumulator (AL/AX) to/from direct memory offset (A0-A3)
            0xA0..=0xA3 => self.mov_acc_moffs(opcode, memory),

            // MOVS - Move String (A4-A5)
            0xA4..=0xA5 => self.movs(opcode, memory),

            // CMPS - Compare String (A6-A7)
            0xA6..=0xA7 => self.cmps(opcode, memory),

            // TEST immediate to AL/AX (A8-A9)
            0xA8..=0xA9 => self.test_imm_acc(opcode, memory),

            // STOS - Store String (AA-AB)
            0xAA..=0xAB => self.stos(opcode, memory),

            // LODS - Load String (AC-AD)
            0xAC..=0xAD => self.lods(opcode, memory),

            // SCAS - Scan String (AE-AF)
            0xAE..=0xAF => self.scas(opcode, memory),

            // MOV immediate to register (B0-BF)
            0xB0..=0xBF => self.mov_imm_to_reg(opcode, memory),

            // Shift/Rotate Group 2 with immediate (C0: 8-bit, C1: 16-bit) - 80186+
            0xC0..=0xC1 => self.shift_rotate_group(opcode, memory),

            // MOV immediate to r/m (C6: 8-bit, C7: 16-bit)
            0xC6..=0xC7 => self.mov_imm_to_rm(opcode, memory),

            // RET with optional pop (C2: with imm16, C3: without)
            0xC2..=0xC3 => self.ret(opcode, memory),

            // Shift/Rotate Group 2 (D0: r/m8, 1; D1: r/m16, 1; D2: r/m8, CL; D3: r/m16, CL)
            0xD0..=0xD3 => self.shift_rotate_group(opcode, memory),

            // CALL near relative (E8)
            0xE8 => self.call_near(memory),

            // JMP near relative (E9)
            0xE9 => self.jmp_near(memory),

            // JMP short relative (EB)
            0xEB => self.jmp_short(memory),

            // HLT - Halt (F4)
            0xF4 => self.hlt(),

            // NOT/NEG/MUL/DIV Group 3 (F6: 8-bit, F7: 16-bit)
            0xF6..=0xF7 => self.unary_group3(opcode, memory),

            // CLD - Clear Direction Flag (FC)
            0xFC => self.cld(),

            // STD - Set Direction Flag (FD)
            0xFD => self.std_flag(),

            // INC/DEC r/m (FE: 8-bit, FF: 16-bit)
            0xFE..=0xFF => self.inc_dec_rm(opcode, memory),

            _ => {
                panic!("Unknown opcode: {:#04X} at {:04X}:{:04X}", opcode, self.cs, self.ip.wrapping_sub(1));
            }
        }
    }

    // Set 8-bit register
    pub(super) fn set_reg8(&mut self, reg: u8, value: u8) {
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

    // Get 8-bit register value
    pub(super) fn get_reg8(&self, reg: u8) -> u8 {
        match reg {
            0 => (self.ax & 0xFF) as u8, // AL
            1 => (self.cx & 0xFF) as u8, // CL
            2 => (self.dx & 0xFF) as u8, // DL
            3 => (self.bx & 0xFF) as u8, // BL
            4 => (self.ax >> 8) as u8, // AH
            5 => (self.cx >> 8) as u8, // CH
            6 => (self.dx >> 8) as u8, // DH
            7 => (self.bx >> 8) as u8, // BH
            _ => unreachable!(),
        }
    }

    // Get 16-bit register value
    pub(super) fn get_reg16(&self, reg: u8) -> u16 {
        match reg & 0x07 {
            0 => self.ax,
            1 => self.cx,
            2 => self.dx,
            3 => self.bx,
            4 => self.sp,
            5 => self.bp,
            6 => self.si,
            7 => self.di,
            _ => unreachable!(),
        }
    }

    // Set a specific flag
    pub(super) fn set_flag(&mut self, flag: u16, value: bool) {
        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    // Get a specific flag
    pub(super) fn get_flag(&self, flag: u16) -> bool {
        (self.flags & flag) != 0
    }

    // Push 16-bit value onto stack
    pub(super) fn push(&mut self, value: u16, memory: &mut Memory) {
        self.sp = self.sp.wrapping_sub(2);
        let addr = Self::physical_address(self.ss, self.sp);
        memory.write_word(addr, value);
    }

    // Pop 16-bit value from stack
    pub(super) fn pop(&mut self, memory: &Memory) -> u16 {
        let addr = Self::physical_address(self.ss, self.sp);
        let value = memory.read_word(addr);
        self.sp = self.sp.wrapping_add(2);
        value
    }

    // Decode ModR/M byte and calculate effective address
    // Returns (mod, reg, r/m, effective_address, default_segment)
    // mod: 00=no disp (except r/m=110), 01=8-bit disp, 10=16-bit disp, 11=register
    // For mod=11, effective_address is unused
    pub(super) fn decode_modrm(&mut self, modrm: u8, memory: &Memory) -> (u8, u8, u8, usize, u16) {
        let mode = modrm >> 6;
        let reg = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;

        if mode == 0b11 {
            // Register mode - no memory access
            return (mode, reg, rm, 0, self.ds);
        }

        // Calculate base address from r/m field
        let (base_addr, default_seg) = match rm {
            0b000 => (self.bx.wrapping_add(self.si), self.ds),  // [BX + SI]
            0b001 => (self.bx.wrapping_add(self.di), self.ds),  // [BX + DI]
            0b010 => (self.bp.wrapping_add(self.si), self.ss),  // [BP + SI]
            0b011 => (self.bp.wrapping_add(self.di), self.ss),  // [BP + DI]
            0b100 => (self.si, self.ds),                         // [SI]
            0b101 => (self.di, self.ds),                         // [DI]
            0b110 => {
                if mode == 0b00 {
                    // Special case: direct address (16-bit displacement, no base)
                    let disp = self.fetch_word(memory);
                    return (mode, reg, rm, Self::physical_address(self.ds, disp), self.ds);
                } else {
                    (self.bp, self.ss)  // [BP]
                }
            }
            0b111 => (self.bx, self.ds),                         // [BX]
            _ => unreachable!(),
        };

        // Add displacement based on mode
        let effective_offset = match mode {
            0b00 => base_addr,  // No displacement
            0b01 => {
                // 8-bit signed displacement
                let disp = self.fetch_byte(memory) as i8;
                base_addr.wrapping_add(disp as i16 as u16)
            }
            0b10 => {
                // 16-bit displacement
                let disp = self.fetch_word(memory);
                base_addr.wrapping_add(disp)
            }
            _ => unreachable!(),
        };

        let effective_addr = Self::physical_address(default_seg, effective_offset);
        (mode, reg, rm, effective_addr, default_seg)
    }

    // Read 8-bit value from register or memory based on mod field
    pub(super) fn read_rm8(&self, mode: u8, rm: u8, addr: usize, memory: &Memory) -> u8 {
        if mode == 0b11 {
            // Register mode
            self.get_reg8(rm)
        } else {
            // Memory mode
            memory.read_byte(addr)
        }
    }

    // Read 16-bit value from register or memory based on mod field
    pub(super) fn read_rm16(&self, mode: u8, rm: u8, addr: usize, memory: &Memory) -> u16 {
        if mode == 0b11 {
            // Register mode
            self.get_reg16(rm)
        } else {
            // Memory mode
            memory.read_word(addr)
        }
    }

    // Write 8-bit value to register or memory based on mod field
    pub(super) fn write_rm8(&mut self, mode: u8, rm: u8, addr: usize, value: u8, memory: &mut Memory) {
        if mode == 0b11 {
            // Register mode
            self.set_reg8(rm, value);
        } else {
            // Memory mode
            memory.write_byte(addr, value);
        }
    }

    // Write 16-bit value to register or memory based on mod field
    pub(super) fn write_rm16(&mut self, mode: u8, rm: u8, addr: usize, value: u16, memory: &mut Memory) {
        if mode == 0b11 {
            // Register mode
            self.set_reg16(rm, value);
        } else {
            // Memory mode
            memory.write_word(addr, value);
        }
    }

    // Calculate and set flags for 8-bit result
    pub(super) fn set_flags_8(&mut self, result: u8) {
        self.set_flag(FLAG_ZERO, result == 0);
        self.set_flag(FLAG_SIGN, (result & 0x80) != 0);
        self.set_flag(FLAG_PARITY, result.count_ones() % 2 == 0);
    }

    // Calculate and set flags for 16-bit result
    pub(super) fn set_flags_16(&mut self, result: u16) {
        self.set_flag(FLAG_ZERO, result == 0);
        self.set_flag(FLAG_SIGN, (result & 0x8000) != 0);
        self.set_flag(FLAG_PARITY, (result as u8).count_ones() % 2 == 0);
    }

    // Dump register state
    pub fn dump_registers(&self) {
        println!("AX={:04X}  BX={:04X}  CX={:04X}  DX={:04X}", self.ax, self.bx, self.cx, self.dx);
        println!("SI={:04X}  DI={:04X}  BP={:04X}  SP={:04X}", self.si, self.di, self.bp, self.sp);
        println!("CS={:04X}  DS={:04X}  SS={:04X}  ES={:04X}", self.cs, self.ds, self.ss, self.es);
        println!("IP={:04X}  FLAGS={:04X}", self.ip, self.flags);
        println!("CF={}  PF={}  AF={}  ZF={}  SF={}  TF={}  IF={}  DF={}  OF={}",
            if self.get_flag(FLAG_CARRY) { 1 } else { 0 },
            if self.get_flag(FLAG_PARITY) { 1 } else { 0 },
            if self.get_flag(FLAG_AUXILIARY) { 1 } else { 0 },
            if self.get_flag(FLAG_ZERO) { 1 } else { 0 },
            if self.get_flag(FLAG_SIGN) { 1 } else { 0 },
            if self.get_flag(FLAG_TRAP) { 1 } else { 0 },
            if self.get_flag(FLAG_INTERRUPT) { 1 } else { 0 },
            if self.get_flag(FLAG_DIRECTION) { 1 } else { 0 },
            if self.get_flag(FLAG_OVERFLOW) { 1 } else { 0 },
        );
    }
}
