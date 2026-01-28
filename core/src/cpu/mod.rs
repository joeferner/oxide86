use crate::{IoDevice, io_port::IoPort, memory::Memory};

pub mod bios;
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
    pub fs: u16, // 80386+
    pub gs: u16, // 80386+

    // Instruction pointer
    pub ip: u16,

    // Flags (start with just carry, zero, sign)
    pub flags: u16,

    // Halted flag
    halted: bool,

    // Segment override prefix (for next instruction only)
    segment_override: Option<u16>,

    // Repeat prefix for string instructions
    repeat_prefix: Option<RepeatPrefix>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub(super) enum RepeatPrefix {
    Rep,   // 0xF3 - Repeat while CX != 0
    Repe,  // 0xF3 - Repeat while CX != 0 and ZF = 1
    Repne, // 0xF2 - Repeat while CX != 0 and ZF = 0
}

// Flag bit positions
pub mod cpu_flag {
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
            segment_override: None,
            repeat_prefix: None,
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
        self.segment_override = None;
        self.repeat_prefix = None;
        // Other registers are undefined on reset
    }

    // Calculate physical address from segment:offset
    pub fn physical_address(segment: u16, offset: u16) -> usize {
        ((segment as usize) << 4) + (offset as usize)
    }

    // Fetch a byte from memory at CS:IP and increment IP
    pub(crate) fn fetch_byte(&mut self, memory: &Memory) -> u8 {
        let addr = Self::physical_address(self.cs, self.ip);
        let byte = memory.read_u8(addr);
        self.ip = self.ip.wrapping_add(1);
        byte
    }

    // Fetch a word (2 bytes, little-endian) from memory at CS:IP
    pub(super) fn fetch_word(&mut self, memory: &Memory) -> u16 {
        let low = self.fetch_byte(memory) as u16;
        let high = self.fetch_byte(memory) as u16;
        (high << 8) | low
    }

    // Check if CPU is halted
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    // Execute an INT instruction with BIOS I/O handler
    pub fn execute_int_with_io<T: crate::cpu::bios::Bios>(
        &mut self,
        int_num: u8,
        memory: &mut Memory,
        io: &mut T,
        video: &mut crate::video::Video,
    ) {
        let is_bios_handler = Self::is_bios_handler(memory, int_num);

        if int_num != 0x10
            && int_num != 0x16
            && int_num != 0x1a
            && int_num != 0x2a
            && int_num != 0x28
            && int_num != 0x29
        {
            log::trace!(
                "INT 0x{:02X} AX={:04X} BX={:04X} CX={:04X} DX={:04X} BIOS={}",
                int_num,
                self.ax,
                self.bx,
                self.cx,
                self.dx,
                is_bios_handler
            );
        }

        // If DOS has installed its own handler (IVT not pointing to BIOS ROM),
        // let DOS handle it instead of intercepting
        if is_bios_handler {
            self.handle_bios_interrupt_impl(int_num, memory, io, video);
        } else {
            // Not handled, do normal INT
            // Push flags, CS, and IP
            self.push(self.flags, memory);
            self.push(self.cs, memory);
            self.push(self.ip, memory);
            // Clear IF and TF
            self.set_flag(cpu_flag::INTERRUPT, false);
            self.set_flag(cpu_flag::TRAP, false);
            // Load interrupt vector from IVT
            let ivt_addr = (int_num as usize) * 4;
            let offset = memory.read_u16(ivt_addr);
            let segment = memory.read_u16(ivt_addr + 2);
            self.ip = offset;
            self.cs = segment;
        }
    }

    // Decode and execute instruction with I/O port support
    pub(crate) fn execute_with_io<I: IoDevice>(
        &mut self,
        opcode: u8,
        memory: &mut Memory,
        io_port: &mut IoPort<I>,
    ) {
        match opcode {
            // ADD r/m to register
            0x00..=0x03 => self.add_rm_reg(opcode, memory),

            // ADD immediate to AL/AX
            0x04..=0x05 => self.add_imm_acc(opcode, memory),

            // PUSH ES (06)
            0x06 => self.push_segreg(opcode, memory),

            // POP ES (07)
            0x07 => self.pop_segreg(opcode, memory),

            // OR r/m to register
            0x08..=0x0B => self.or_rm_reg(opcode, memory),

            // OR immediate to AL/AX
            0x0C..=0x0D => self.or_imm_acc(opcode, memory),

            // PUSH CS (0E)
            0x0E => self.push_segreg(opcode, memory),

            // ADC r/m to register (10-13)
            0x10..=0x13 => self.adc_rm_reg(opcode, memory),

            // ADC immediate to AL/AX (14-15)
            0x14..=0x15 => self.adc_imm_acc(opcode, memory),

            // PUSH SS (16)
            0x16 => self.push_segreg(opcode, memory),

            // POP SS (17)
            0x17 => self.pop_segreg(opcode, memory),

            // SBB r/m to register (18-1B)
            0x18..=0x1B => self.sbb_rm_reg(opcode, memory),

            // SBB immediate to AL/AX (1C-1D)
            0x1C..=0x1D => self.sbb_imm_acc(opcode, memory),

            // PUSH DS (1E)
            0x1E => self.push_segreg(opcode, memory),

            // POP DS (1F)
            0x1F => self.pop_segreg(opcode, memory),

            // AND r/m to register
            0x20..=0x23 => self.and_rm_reg(opcode, memory),

            // AND immediate to AL/AX
            0x24..=0x25 => self.and_imm_acc(opcode, memory),

            // ES: segment override prefix (26)
            0x26 => {
                self.segment_override = Some(self.es);
                let next_opcode = self.fetch_byte(memory);
                self.execute_with_io(next_opcode, memory, io_port);
                self.segment_override = None;
            }

            // DAA - Decimal Adjust After Addition (27)
            0x27 => self.daa(),

            // SUB r/m to register
            0x28..=0x2B => self.sub_rm_reg(opcode, memory),

            // SUB immediate to AL/AX
            0x2C..=0x2D => self.sub_imm_acc(opcode, memory),

            // CS: segment override prefix (2E)
            0x2E => {
                self.segment_override = Some(self.cs);
                let next_opcode = self.fetch_byte(memory);
                self.execute_with_io(next_opcode, memory, io_port);
                self.segment_override = None;
            }

            // DAS - Decimal Adjust After Subtraction (2F)
            0x2F => self.das(),

            // XOR r/m to register
            0x30..=0x33 => self.xor_rm_reg(opcode, memory),

            // XOR immediate to AL/AX
            0x34..=0x35 => self.xor_imm_acc(opcode, memory),

            // SS: segment override prefix (36)
            0x36 => {
                self.segment_override = Some(self.ss);
                let next_opcode = self.fetch_byte(memory);
                self.execute_with_io(next_opcode, memory, io_port);
                self.segment_override = None;
            }

            // AAA - ASCII Adjust After Addition (37)
            0x37 => self.aaa(),

            // CMP r/m to register
            0x38..=0x3B => self.cmp_rm_reg(opcode, memory),

            // CMP immediate to AL/AX
            0x3C..=0x3D => self.cmp_imm_acc(opcode, memory),

            // DS: segment override prefix (3E)
            0x3E => {
                self.segment_override = Some(self.ds);
                let next_opcode = self.fetch_byte(memory);
                self.execute_with_io(next_opcode, memory, io_port);
                self.segment_override = None;
            }

            // AAS - ASCII Adjust After Subtraction (3F)
            0x3F => self.aas(),

            // INC 16-bit register (40-47)
            0x40..=0x47 => self.inc_reg16(opcode),

            // DEC 16-bit register (48-4F)
            0x48..=0x4F => self.dec_reg16(opcode),

            // PUSH 16-bit register (50-57)
            0x50..=0x57 => self.push_reg16(opcode, memory),

            // POP 16-bit register (58-5F)
            0x58..=0x5F => self.pop_reg16(opcode, memory),

            // PUSHA - Push All General Registers (60)
            0x60 => self.pusha(memory),

            // BOUND - Check Array Index Against Bounds (62)
            0x62 => {
                if self.bound(memory) {
                    // Index out of bounds - trigger INT 5
                    self.push(self.flags, memory);
                    self.push(self.cs, memory);
                    // IP points after BOUND instruction; we need to point at it
                    // The modrm byte and any displacement were already consumed by bound()
                    // so we save the current IP (which is past the instruction)
                    self.push(self.ip, memory);
                    self.set_flag(cpu_flag::INTERRUPT, false);
                    self.set_flag(cpu_flag::TRAP, false);
                    // Load INT 5 vector
                    let ivt_addr = 5 * 4;
                    self.ip = memory.read_u16(ivt_addr);
                    self.cs = memory.read_u16(ivt_addr + 2);
                }
            }

            // FS: segment override prefix (64) - 80386+
            0x64 => {
                self.segment_override = Some(self.fs);
                let next_opcode = self.fetch_byte(memory);
                self.execute_with_io(next_opcode, memory, io_port);
                self.segment_override = None;
            }

            // GS: segment override prefix (65) - 80386+
            0x65 => {
                self.segment_override = Some(self.gs);
                let next_opcode = self.fetch_byte(memory);
                self.execute_with_io(next_opcode, memory, io_port);
                self.segment_override = None;
            }

            // PUSH immediate (68: imm16, 6A: imm8 sign-extended)
            0x68 | 0x6A => self.push_imm(opcode, memory),

            // IMUL - Signed Multiply with Immediate (69: imm16)
            0x69 => self.imul_imm16(memory),

            // INS - Input String from Port (6C-6D)
            0x6C..=0x6D => self.ins(opcode, memory, io_port),

            // OUTS - Output String to Port (6E-6F)
            0x6E..=0x6F => self.outs(opcode, memory, io_port),

            // Conditional jumps (70-7F)
            0x70..=0x7F => self.jmp_conditional(opcode, memory),

            // Arithmetic/logical immediate to r/m (80: 8-bit, 81: 16-bit, 83: sign-extended 8-bit to 16-bit)
            0x80 => self.arith_imm8_rm8(memory),
            0x81 => self.arith_imm16_rm(memory),
            0x83 => self.arith_imm8_rm(memory),

            // TEST r/m and register (84-85)
            0x84..=0x85 => self.test_rm_reg(opcode, memory),

            // XCHG r/m and register (86-87)
            0x86..=0x87 => self.xchg_rm_reg(opcode, memory),

            // MOV register to/from r/m (88-8B)
            0x88..=0x8B => self.mov_reg_rm(opcode, memory),

            // MOV segment register to r/m16 (8C)
            0x8C => self.mov_segreg_to_rm(memory),

            // LEA - Load Effective Address (8D)
            0x8D => self.lea(memory),

            // MOV r/m16 to segment register (8E)
            0x8E => self.mov_rm_to_segreg(memory),

            // POP r/m16 (8F) - Group 1A
            0x8F => self.pop_rm16(memory),

            // NOP / XCHG AX, reg (90-97)
            0x90..=0x97 => self.xchg_ax_reg(opcode),

            // CBW - Convert Byte to Word (98)
            0x98 => self.cbw(),

            // CWD - Convert Word to Double word (99)
            0x99 => self.cwd(),

            // CALL far (9A)
            0x9A => self.call_far(memory),

            // PUSHF - Push Flags (9C)
            0x9C => self.pushf(memory),

            // POPF - Pop Flags (9D)
            0x9D => self.popf(memory),

            // SAHF - Store AH into Flags (9E)
            0x9E => self.sahf(),

            // LAHF - Load AH from Flags (9F)
            0x9F => self.lahf(),

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

            // RET with optional pop (C2: with imm16, C3: without)
            0xC2..=0xC3 => self.ret(opcode, memory),

            // LES - Load Pointer using ES (C4)
            0xC4 => self.les(memory),

            // LDS - Load Pointer using DS (C5)
            0xC5 => self.lds(memory),

            // MOV immediate to r/m (C6: 8-bit, C7: 16-bit)
            0xC6..=0xC7 => self.mov_imm_to_rm(opcode, memory),

            // RET far (CA: with imm16, CB: without)
            0xCA..=0xCB => self.retf(opcode, memory),

            // INT 3 - Breakpoint (CC)
            0xCC => self.int3(memory),

            // INT - Software Interrupt (CD)
            0xCD => self.int(memory),

            // INTO - Interrupt on Overflow (CE)
            0xCE => self.into(memory),

            // IRET - Interrupt Return (CF)
            0xCF => self.iret(memory),

            // Shift/Rotate Group 2 (D0: r/m8, 1; D1: r/m16, 1; D2: r/m8, CL; D3: r/m16, CL)
            0xD0..=0xD3 => self.shift_rotate_group(opcode, memory),

            // AAM - ASCII Adjust After Multiplication (D4)
            0xD4 => self.aam(memory),

            // AAD - ASCII Adjust Before Division (D5)
            0xD5 => self.aad(memory),

            // XLAT - Table Look-up Translation (D7)
            0xD7 => self.xlat(memory),

            // ESC - Escape to coprocessor (D8-DF)
            // Passes instruction to 8087 FPU; NOP without coprocessor
            0xD8..=0xDF => self.esc(memory),

            // LOOPNE/LOOPNZ (E0)
            0xE0 => self.loopne(memory),

            // LOOPE/LOOPZ (E1)
            0xE1 => self.loope(memory),

            // LOOP (E2)
            0xE2 => self.loop_inst(memory),

            // JCXZ (E3)
            0xE3 => self.jcxz(memory),

            // IN AL, imm8 (E4)
            0xE4 => self.in_al_imm8(memory, io_port),

            // IN AX, imm8 (E5)
            0xE5 => self.in_ax_imm8(memory, io_port),

            // OUT imm8, AL (E6)
            0xE6 => self.out_imm8_al(memory, io_port),

            // OUT imm8, AX (E7)
            0xE7 => self.out_imm8_ax(memory, io_port),

            // CALL near relative (E8)
            0xE8 => self.call_near(memory),

            // JMP near relative (E9)
            0xE9 => self.jmp_near(memory),

            // JMP far (EA)
            0xEA => self.jmp_far(memory),

            // JMP short relative (EB)
            0xEB => self.jmp_short(memory),

            // IN AL, DX (EC)
            0xEC => self.in_al_dx(io_port),

            // IN AX, DX (ED)
            0xED => self.in_ax_dx(io_port),

            // OUT DX, AL (EE)
            0xEE => self.out_dx_al(io_port),

            // OUT DX, AX (EF)
            0xEF => self.out_dx_ax(io_port),

            // LOCK prefix (F0)
            // Asserts LOCK# signal for atomic memory operations; no-op in single-processor emulator
            0xF0 => {
                let next_opcode = self.fetch_byte(memory);
                self.execute_with_io(next_opcode, memory, io_port);
            }

            // REPNE/REPNZ prefix (F2)
            0xF2 => {
                self.repeat_prefix = Some(RepeatPrefix::Repne);
                let next_opcode = self.fetch_byte(memory);
                self.execute_with_io(next_opcode, memory, io_port);
                self.repeat_prefix = None;
            }

            // REP/REPE/REPZ prefix (F3)
            0xF3 => {
                self.repeat_prefix = Some(RepeatPrefix::Rep);
                let next_opcode = self.fetch_byte(memory);
                self.execute_with_io(next_opcode, memory, io_port);
                self.repeat_prefix = None;
            }

            // HLT - Halt (F4)
            0xF4 => self.hlt(),

            // CMC - Complement Carry Flag (F5)
            0xF5 => self.cmc(),

            // NOT/NEG/MUL/DIV Group 3 (F6: 8-bit, F7: 16-bit)
            0xF6..=0xF7 => self.unary_group3(opcode, memory),

            // CLC - Clear Carry Flag (F8)
            0xF8 => self.clc(),

            // STC - Set Carry Flag (F9)
            0xF9 => self.stc(),

            // CLI - Clear Interrupt Flag (FA)
            0xFA => self.cli(),

            // STI - Set Interrupt Flag (FB)
            0xFB => self.sti(),

            // CLD - Clear Direction Flag (FC)
            0xFC => self.cld(),

            // STD - Set Direction Flag (FD)
            0xFD => self.std_flag(),

            // INC/DEC/CALL/JMP Group 4/5 (FE: 8-bit, FF: 16-bit)
            0xFE => self.inc_dec_rm(opcode, memory),
            0xFF => {
                // For FF, we need to check the reg field to determine operation
                let modrm_peek = memory.read_u8(Self::physical_address(self.cs, self.ip));
                let reg_field = (modrm_peek >> 3) & 0x07;
                match reg_field {
                    0 | 1 => self.inc_dec_rm(opcode, memory), // INC/DEC
                    2 | 3 => self.call_indirect(memory),      // CALL near/far
                    4 | 5 => self.jmp_indirect(memory),       // JMP near/far
                    6 => self.push_rm16(memory),              // PUSH r/m16
                    _ => panic!("Invalid FF operation: {}", reg_field),
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
            4 => (self.ax >> 8) as u8,   // AH
            5 => (self.cx >> 8) as u8,   // CH
            6 => (self.dx >> 8) as u8,   // DH
            7 => (self.bx >> 8) as u8,   // BH
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

    // Get segment register value
    pub(super) fn get_segreg(&self, reg: u8) -> u16 {
        match reg & 0x03 {
            0 => self.es,
            1 => self.cs,
            2 => self.ss,
            3 => self.ds,
            _ => unreachable!(),
        }
    }

    // Set segment register value
    pub(super) fn set_segreg(&mut self, reg: u8, value: u16) {
        match reg & 0x03 {
            0 => self.es = value,
            1 => self.cs = value,
            2 => self.ss = value,
            3 => self.ds = value,
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
        memory.write_u16(addr, value);
    }

    // Pop 16-bit value from stack
    pub(super) fn pop(&mut self, memory: &Memory) -> u16 {
        let addr = Self::physical_address(self.ss, self.sp);
        let value = memory.read_u16(addr);
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
            0b000 => (self.bx.wrapping_add(self.si), self.ds), // [BX + SI]
            0b001 => (self.bx.wrapping_add(self.di), self.ds), // [BX + DI]
            0b010 => (self.bp.wrapping_add(self.si), self.ss), // [BP + SI]
            0b011 => (self.bp.wrapping_add(self.di), self.ss), // [BP + DI]
            0b100 => (self.si, self.ds),                       // [SI]
            0b101 => (self.di, self.ds),                       // [DI]
            0b110 => {
                if mode == 0b00 {
                    // Special case: direct address (16-bit displacement, no base)
                    let disp = self.fetch_word(memory);
                    let seg = self.segment_override.unwrap_or(self.ds);
                    return (mode, reg, rm, Self::physical_address(seg, disp), seg);
                } else {
                    (self.bp, self.ss) // [BP]
                }
            }
            0b111 => (self.bx, self.ds), // [BX]
            _ => unreachable!(),
        };

        // Add displacement based on mode
        let effective_offset = match mode {
            0b00 => base_addr, // No displacement
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

        // Use segment override if present, otherwise use default segment
        let effective_seg = self.segment_override.unwrap_or(default_seg);
        let effective_addr = Self::physical_address(effective_seg, effective_offset);
        (mode, reg, rm, effective_addr, effective_seg)
    }

    // Read 8-bit value from register or memory based on mod field
    pub(super) fn read_rm8(&self, mode: u8, rm: u8, addr: usize, memory: &Memory) -> u8 {
        if mode == 0b11 {
            // Register mode
            self.get_reg8(rm)
        } else {
            // Memory mode
            memory.read_u8(addr)
        }
    }

    // Read 16-bit value from register or memory based on mod field
    pub(super) fn read_rm16(&self, mode: u8, rm: u8, addr: usize, memory: &Memory) -> u16 {
        if mode == 0b11 {
            // Register mode
            self.get_reg16(rm)
        } else {
            // Memory mode
            memory.read_u16(addr)
        }
    }

    // Write 8-bit value to register or memory based on mod field
    pub(super) fn write_rm8(
        &mut self,
        mode: u8,
        rm: u8,
        addr: usize,
        value: u8,
        memory: &mut Memory,
    ) {
        if mode == 0b11 {
            // Register mode
            self.set_reg8(rm, value);
        } else {
            // Memory mode
            memory.write_u8(addr, value);
        }
    }

    // Write 16-bit value to register or memory based on mod field
    pub(super) fn write_rm16(
        &mut self,
        mode: u8,
        rm: u8,
        addr: usize,
        value: u16,
        memory: &mut Memory,
    ) {
        if mode == 0b11 {
            // Register mode
            self.set_reg16(rm, value);
        } else {
            // Memory mode
            memory.write_u16(addr, value);
        }
    }

    // Calculate and set flags for 8-bit result
    pub(super) fn set_flags_8(&mut self, result: u8) {
        self.set_flag(cpu_flag::ZERO, result == 0);
        self.set_flag(cpu_flag::SIGN, (result & 0x80) != 0);
        self.set_flag(cpu_flag::PARITY, result.count_ones().is_multiple_of(2));
    }

    // Calculate and set flags for 16-bit result
    pub(super) fn set_flags_16(&mut self, result: u16) {
        self.set_flag(cpu_flag::ZERO, result == 0);
        self.set_flag(cpu_flag::SIGN, (result & 0x8000) != 0);
        self.set_flag(
            cpu_flag::PARITY,
            (result as u8).count_ones().is_multiple_of(2),
        );
    }

    // Dump register state
    pub fn dump_registers(&self) {
        log::info!(
            "AX={:04X}  BX={:04X}  CX={:04X}  DX={:04X}",
            self.ax,
            self.bx,
            self.cx,
            self.dx
        );
        log::info!(
            "SI={:04X}  DI={:04X}  BP={:04X}  SP={:04X}",
            self.si,
            self.di,
            self.bp,
            self.sp
        );
        log::info!(
            "CS={:04X}  DS={:04X}  SS={:04X}  ES={:04X}",
            self.cs,
            self.ds,
            self.ss,
            self.es
        );
        log::info!("IP={:04X}  FLAGS={:04X}", self.ip, self.flags);
        log::info!(
            "CF={}  PF={}  AF={}  ZF={}  SF={}  TF={}  IF={}  DF={}  OF={}",
            if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 },
            if self.get_flag(cpu_flag::PARITY) {
                1
            } else {
                0
            },
            if self.get_flag(cpu_flag::AUXILIARY) {
                1
            } else {
                0
            },
            if self.get_flag(cpu_flag::ZERO) { 1 } else { 0 },
            if self.get_flag(cpu_flag::SIGN) { 1 } else { 0 },
            if self.get_flag(cpu_flag::TRAP) { 1 } else { 0 },
            if self.get_flag(cpu_flag::INTERRUPT) {
                1
            } else {
                0
            },
            if self.get_flag(cpu_flag::DIRECTION) {
                1
            } else {
                0
            },
            if self.get_flag(cpu_flag::OVERFLOW) {
                1
            } else {
                0
            },
        );
    }
}
