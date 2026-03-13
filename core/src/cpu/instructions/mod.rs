use crate::{
    bus::Bus,
    cpu::{Cpu, cpu_flag},
    physical_address,
};

mod arithmetic;
mod comparison;
mod control_flow;
mod data_transfer;
pub(crate) mod decoder;
mod io;
mod logical;
mod shift_rotate;
mod string;

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub(in crate::cpu) enum RepeatPrefix {
    Rep,   // 0xF3 - Repeat while CX != 0
    Repe,  // 0xF3 - Repeat while CX != 0 and ZF = 1
    Repne, // 0xF2 - Repeat while CX != 0 and ZF = 0
}

impl Cpu {
    pub(in crate::cpu) fn exec_instruction(&mut self, bus: &mut Bus) {
        let opcode = self.fetch_byte(bus);
        match opcode {
            // ADD r/m to register
            0x00..=0x03 => self.add_rm_reg(opcode, bus),

            // ADD immediate to AL/AX
            0x04..=0x05 => self.add_imm_acc(opcode, bus),

            // PUSH ES (06)
            0x06 => self.push_segreg(opcode, bus),

            // POP ES (07)
            0x07 => self.pop_segreg(opcode, bus),

            // OR r/m to register
            0x08..=0x0B => self.or_rm_reg(opcode, bus),

            // OR immediate to AL/AX
            0x0C..=0x0D => self.or_imm_acc(opcode, bus),

            // PUSH CS (0E)
            0x0E => self.push_segreg(opcode, bus),

            // POP CS (0F) - 8086 only, repurposed as two-byte prefix on 80286+
            0x0F => {
                log::warn!(
                    "POP CS at {:04X}:{:04X} (8086 instruction, dangerous!)",
                    self.cs,
                    self.ip.wrapping_sub(1)
                );
                self.pop_segreg(opcode, bus);
            }

            // ADC r/m to register (10-13)
            0x10..=0x13 => self.adc_rm_reg(opcode, bus),

            // ADC immediate to AL/AX (14-15)
            0x14..=0x15 => self.adc_imm_acc(opcode, bus),

            // PUSH SS (16)
            0x16 => self.push_segreg(opcode, bus),

            // POP SS (17)
            0x17 => self.pop_segreg(opcode, bus),

            // SBB r/m to register (18-1B)
            0x18..=0x1B => self.sbb_rm_reg(opcode, bus),

            // SBB immediate to AL/AX (1C-1D)
            0x1C..=0x1D => self.sbb_imm_acc(opcode, bus),

            // PUSH DS (1E)
            0x1E => self.push_segreg(opcode, bus),

            // POP DS (1F)
            0x1F => self.pop_segreg(opcode, bus),

            // AND r/m to register
            0x20..=0x23 => self.and_rm_reg(opcode, bus),

            // AND immediate to AL/AX
            0x24..=0x25 => self.and_imm_acc(opcode, bus),

            // ES: segment override prefix (26)
            0x26 => {
                self.segment_override = Some(self.es);
                self.exec_instruction(bus);
                self.segment_override = None;
            }

            // DAA - Decimal Adjust After Addition (27)
            0x27 => self.daa(bus),

            // SUB r/m to register
            0x28..=0x2B => self.sub_rm_reg(opcode, bus),

            // SUB immediate to AL/AX
            0x2C..=0x2D => self.sub_imm_acc(opcode, bus),

            // CS: segment override prefix (2E)
            0x2E => {
                self.segment_override = Some(self.cs);
                self.exec_instruction(bus);
                self.segment_override = None;
            }

            // DAS - Decimal Adjust After Subtraction (2F)
            0x2F => self.das(bus),

            // XOR r/m to register
            0x30..=0x33 => self.xor_rm_reg(opcode, bus),

            // XOR immediate to AL/AX
            0x34..=0x35 => self.xor_imm_acc(opcode, bus),

            // SS: segment override prefix (36)
            0x36 => {
                self.segment_override = Some(self.ss);
                self.exec_instruction(bus);
                self.segment_override = None;
            }

            // AAA - ASCII Adjust After Addition (37)
            0x37 => self.aaa(bus),

            // CMP r/m to register
            0x38..=0x3B => self.cmp_rm_reg(opcode, bus),

            // CMP immediate to AL/AX
            0x3C..=0x3D => self.cmp_imm_acc(opcode, bus),

            // DS: segment override prefix (3E)
            0x3E => {
                self.segment_override = Some(self.ds);
                self.exec_instruction(bus);
                self.segment_override = None;
            }

            // AAS - ASCII Adjust After Subtraction (3F)
            0x3F => self.aas(bus),

            // INC 16-bit register (40-47)
            0x40..=0x47 => self.inc_reg16(opcode, bus),

            // DEC 16-bit register (48-4F)
            0x48..=0x4F => self.dec_reg16(opcode, bus),

            // PUSH 16-bit register (50-57)
            0x50..=0x57 => self.push_reg16(opcode, bus),

            // POP 16-bit register (58-5F)
            0x58..=0x5F => self.pop_reg16(opcode, bus),

            // PUSHA - Push All General Registers (60) - 286+
            0x60 => {
                if self.cpu_type.is_286_or_later() {
                    self.pusha(bus);
                } else {
                    log::warn!("PUSHA (0x60) not supported on {:?}", self.cpu_type);
                }
            }

            // POPA - Pop All General Registers (61) - 286+
            0x61 => {
                if self.cpu_type.is_286_or_later() {
                    self.popa(bus);
                } else {
                    log::warn!("POPA (0x61) not supported on {:?}", self.cpu_type);
                }
            }

            // BOUND - Check Array Index Against Bounds (62) - 286+
            0x62 => {
                if self.cpu_type.is_286_or_later() {
                    if self.bound(bus) {
                        self.dispatch_interrupt(bus, 5);
                    }
                } else {
                    log::warn!("BOUND (0x62) not supported on {:?}", self.cpu_type);
                }
            }

            // FS: segment override prefix (64) - 80386+
            0x64 => {
                self.segment_override = Some(self.fs);
                self.exec_instruction(bus);
                self.segment_override = None;
            }

            // PUSH immediate (68: imm16, 6A: imm8 sign-extended) - 286+
            0x68 | 0x6A => {
                if self.cpu_type.is_286_or_later() {
                    self.push_imm(opcode, bus);
                } else {
                    log::warn!(
                        "PUSH imm ({:#04X}) not supported on {:?}",
                        opcode,
                        self.cpu_type
                    );
                }
            }

            // IMUL - Signed Multiply with Immediate (69: imm16, 6B: imm8 sign-extended) - 286+
            0x69 => {
                if self.cpu_type.is_286_or_later() {
                    self.imul_imm16(bus);
                } else {
                    log::warn!("IMUL imm16 (0x69) not supported on {:?}", self.cpu_type);
                }
            }
            0x6B => {
                if self.cpu_type.is_286_or_later() {
                    self.imul_imm8(bus);
                } else {
                    log::warn!("IMUL imm8 (0x6B) not supported on {:?}", self.cpu_type);
                }
            }

            // INS - Input String from Port (6C-6D) - 286+
            0x6C..=0x6D => {
                if self.cpu_type.is_286_or_later() {
                    self.ins(opcode, bus);
                } else {
                    log::warn!("INS ({:#04X}) not supported on {:?}", opcode, self.cpu_type);
                }
            }

            // OUTS - Output String to Port (6E-6F) - 286+
            0x6E..=0x6F => {
                if self.cpu_type.is_286_or_later() {
                    self.outs(opcode, bus);
                } else {
                    log::warn!(
                        "OUTS ({:#04X}) not supported on {:?}",
                        opcode,
                        self.cpu_type
                    );
                }
            }

            // Conditional jumps (70-7F)
            0x70..=0x7F => self.jmp_conditional(opcode, bus),

            // Arithmetic/logical immediate to r/m (80: 8-bit, 81: 16-bit, 82: same as 80, 83: sign-extended 8-bit to 16-bit)
            0x80 | 0x82 => self.arith_imm8_rm8(bus),
            0x81 => self.arith_imm16_rm(bus),
            0x83 => self.arith_imm8_rm(bus),

            // TEST r/m and register (84-85)
            0x84..=0x85 => self.test_rm_reg(opcode, bus),

            // XCHG r/m and register (86-87)
            0x86..=0x87 => self.xchg_rm_reg(opcode, bus),

            // MOV register to/from r/m (88-8B)
            0x88..=0x8B => self.mov_reg_rm(opcode, bus),

            // MOV segment register to r/m16 (8C)
            0x8C => self.mov_segreg_to_rm(bus),

            // LEA - Load Effective Address (8D)
            0x8D => self.lea(bus),

            // MOV r/m16 to segment register (8E)
            0x8E => self.mov_rm_to_segreg(bus),

            // POP r/m16 (8F) - Group 1A
            0x8F => self.pop_rm16(bus),

            // NOP / XCHG AX, reg (90-97)
            0x90..=0x97 => self.xchg_ax_reg(opcode, bus),

            // CBW - Convert Byte to Word (98)
            0x98 => self.cbw(bus),

            // CWD - Convert Word to Double word (99)
            0x99 => self.cwd(bus),

            // CALL far (9A)
            0x9A => self.call_far(bus),

            // PUSHF - Push Flags (9C)
            0x9C => self.pushf(bus),

            // POPF - Pop Flags (9D)
            0x9D => self.popf(bus),

            // SAHF - Store AH into Flags (9E)
            0x9E => self.sahf(bus),

            // LAHF - Load AH from Flags (9F)
            0x9F => self.lahf(bus),

            // MOV accumulator (AL/AX) to/from direct memory offset (A0-A3)
            0xA0..=0xA3 => self.mov_acc_moffs(opcode, bus),

            // MOVS - Move String (A4-A5)
            0xA4..=0xA5 => self.movs(opcode, bus),

            // CMPS - Compare String (A6-A7)
            0xA6..=0xA7 => self.cmps(opcode, bus),

            // TEST immediate to AL/AX (A8-A9)
            0xA8..=0xA9 => self.test_imm_acc(opcode, bus),

            // STOS - Store String (AA-AB)
            0xAA..=0xAB => self.stos(opcode, bus),

            // LODS - Load String (AC-AD)
            0xAC..=0xAD => self.lods(opcode, bus),

            // SCAS - Scan String (AE-AF)
            0xAE..=0xAF => self.scas(opcode, bus),

            // MOV immediate to register (B0-BF)
            0xB0..=0xBF => self.mov_imm_to_reg(opcode, bus),

            // Shift/Rotate Group 2 with immediate (C0: 8-bit, C1: 16-bit) - 286+
            0xC0..=0xC1 => {
                if self.cpu_type.is_286_or_later() {
                    self.shift_rotate_group(opcode, bus);
                } else {
                    log::warn!(
                        "Shift/rotate by immediate ({:#04X}) not supported on {:?}",
                        opcode,
                        self.cpu_type
                    );
                }
            }

            // RET with optional pop (C2: with imm16, C3: without)
            0xC2..=0xC3 => self.ret(opcode, bus),

            // LES - Load Pointer using ES (C4)
            0xC4 => self.les(bus),

            // LDS - Load Pointer using DS (C5)
            0xC5 => self.lds(bus),

            // MOV immediate to r/m (C6: 8-bit, C7: 16-bit)
            0xC6..=0xC7 => self.mov_imm_to_rm(opcode, bus),

            // ENTER - Make Stack Frame (C8) - 286+
            0xC8 => {
                if self.cpu_type.is_286_or_later() {
                    self.enter(bus);
                } else {
                    log::warn!("ENTER (0xC8) not supported on {:?}", self.cpu_type);
                }
            }

            // LEAVE - High Level Procedure Exit (C9) - 286+
            0xC9 => {
                if self.cpu_type.is_286_or_later() {
                    self.leave(bus);
                } else {
                    log::warn!("LEAVE (0xC9) not supported on {:?}", self.cpu_type);
                }
            }

            // RET far (CA: with imm16, CB: without)
            0xCA..=0xCB => self.retf(opcode, bus),

            // INT 3 - Breakpoint (CC)
            0xCC => self.int3(bus),

            // INT - Software Interrupt (CD)
            0xCD => self.int(bus),

            // IRET - Interrupt Return (CF)
            0xCF => self.iret(bus),

            // Shift/Rotate Group 2 (D0: r/m8, 1; D1: r/m16, 1; D2: r/m8, CL; D3: r/m16, CL)
            0xD0..=0xD3 => self.shift_rotate_group(opcode, bus),

            // AAM - ASCII Adjust After Multiplication (D4)
            0xD4 => self.aam(bus),

            // AAD - ASCII Adjust Before Division (D5)
            0xD5 => self.aad(bus),

            // XLAT - Table Look-up Translation (D7)
            0xD7 => self.xlat(bus),

            // ESC - Escape to coprocessor (D8-DF)
            // Passes instruction to 8087 FPU; NOP without coprocessor
            0xD8..=0xDF => self.esc(bus),

            // LOOPNE/LOOPNZ (E0)
            0xE0 => self.loopne(bus),

            // LOOPE/LOOPZ (E1)
            0xE1 => self.loope(bus),

            // LOOP (E2)
            0xE2 => self.loop_inst(bus),

            // JCXZ (E3)
            0xE3 => self.jcxz(bus),

            // IN AL, imm8 (E4)
            0xE4 => self.in_al_imm8(bus),

            // OUT imm8, AL (E6)
            0xE6 => self.out_imm8_al(bus),

            // CALL near relative (E8)
            0xE8 => self.call_near(bus),

            // JMP near relative (E9)
            0xE9 => self.jmp_near(bus),

            // OUT DX, AL (EE)
            0xEE => self.out_dx_al(bus),

            // JMP far (EA)
            0xEA => self.jmp_far(bus),

            // JMP short relative (EB)
            0xEB => self.jmp_short(bus),

            // IN AL, DX (EC)
            0xEC => self.in_al_dx(bus),

            // IN AX, DX (ED)
            0xED => self.in_ax_dx(bus),

            // LOCK prefix (F0)
            // Asserts LOCK# signal for atomic memory operations; no-op in single-processor emulator
            0xF0 => {
                self.exec_instruction(bus);
            }

            // REPNE/REPNZ prefix (F2)
            0xF2 => {
                self.repeat_prefix = Some(RepeatPrefix::Repne);
                self.exec_instruction(bus);
                self.repeat_prefix = None;
            }

            // REP/REPE/REPZ prefix (F3)
            0xF3 => {
                self.repeat_prefix = Some(RepeatPrefix::Rep);
                self.exec_instruction(bus);
                self.repeat_prefix = None;
            }

            // HLT - Halt (F4)
            0xF4 => self.hlt(bus),

            // CMC - Complement Carry Flag (F5)
            0xF5 => self.cmc(bus),

            // NOT/NEG/MUL/DIV Group 3 (F6: 8-bit, F7: 16-bit)
            0xF6..=0xF7 => self.unary_group3(opcode, bus),

            // CLC - Clear Carry Flag (F8)
            0xF8 => self.clc(bus),

            // STC - Set Carry Flag (F9)
            0xF9 => self.stc(bus),

            // CLI - Clear Interrupt Flag (FA)
            0xFA => self.cli(bus),

            // STI - Set Interrupt Flag (FB)
            0xFB => self.sti(bus),

            // CLD - Clear Direction Flag (FC)
            0xFC => self.cld(bus),

            // STD - Set Direction Flag (FD)
            0xFD => self.std_flag(bus),

            // INC/DEC/CALL/JMP Group 4/5 (FE: 8-bit, FF: 16-bit)
            0xFE => self.inc_dec_rm(opcode, bus),
            0xFF => {
                // For FF, we need to check the reg field to determine operation
                let modrm_peek = bus.memory_read_u8(physical_address(self.cs, self.ip));
                let reg_field = (modrm_peek >> 3) & 0x07;
                match reg_field {
                    0 | 1 => self.inc_dec_rm(opcode, bus), // INC/DEC
                    2 | 3 => self.call_indirect(bus),      // CALL near/far
                    4 | 5 => self.jmp_indirect(bus),       // JMP near/far
                    6 => self.push_rm16(bus),              // PUSH r/m16
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

    // Decode ModR/M byte and calculate effective address
    // Returns (mod, reg, r/m, effective_address, default_segment)
    // mod: 00=no disp (except r/m=110), 01=8-bit disp, 10=16-bit disp, 11=register
    // For mod=11, effective_address is unused
    fn decode_modrm(&mut self, modrm: u8, bus: &Bus) -> (u8, u8, u8, usize, u16) {
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
                    let disp = self.fetch_word(bus);
                    let seg = self.segment_override.unwrap_or(self.ds);
                    return (mode, reg, rm, physical_address(seg, disp), seg);
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
                let disp = self.fetch_byte(bus) as i8;
                base_addr.wrapping_add(disp as i16 as u16)
            }
            0b10 => {
                // 16-bit displacement
                let disp = self.fetch_word(bus);
                base_addr.wrapping_add(disp)
            }
            _ => unreachable!(),
        };

        // Use segment override if present, otherwise use default segment
        let effective_seg = self.segment_override.unwrap_or(default_seg);
        let effective_addr = physical_address(effective_seg, effective_offset);
        (mode, reg, rm, effective_addr, effective_seg)
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

    /// Get 8-bit register value
    pub(in crate::cpu) fn get_reg8(&self, reg: u8) -> u8 {
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

    /// Get 16-bit register value
    fn get_reg16(&self, reg: u8) -> u16 {
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

    /// Set segment register value
    fn set_segreg(&mut self, reg: u8, value: u16) {
        match reg & 0x03 {
            0 => self.es = value,
            1 => self.cs = value,
            2 => self.ss = value,
            3 => self.ds = value,
            _ => unreachable!(),
        }
    }

    /// Calculate and set flags for 8-bit result
    fn set_flags_8(&mut self, result: u8) {
        self.set_flag(cpu_flag::ZERO, result == 0);
        self.set_flag(cpu_flag::SIGN, (result & 0x80) != 0);
        self.set_flag(cpu_flag::PARITY, result.count_ones().is_multiple_of(2));
    }

    /// Calculate and set flags for 16-bit result
    fn set_flags_16(&mut self, result: u16) {
        self.set_flag(cpu_flag::ZERO, result == 0);
        self.set_flag(cpu_flag::SIGN, (result & 0x8000) != 0);
        self.set_flag(
            cpu_flag::PARITY,
            (result as u8).count_ones().is_multiple_of(2),
        );
    }

    /// Read 8-bit value from register or memory based on mod field
    fn read_rm8(&self, mode: u8, rm: u8, addr: usize, bus: &Bus) -> u8 {
        if mode == 0b11 {
            // Register mode
            self.get_reg8(rm)
        } else {
            // Memory mode
            bus.memory_read_u8(addr)
        }
    }

    // Read 16-bit value from register or memory based on mod field
    fn read_rm16(&self, mode: u8, rm: u8, addr: usize, bus: &Bus) -> u16 {
        if mode == 0b11 {
            // Register mode
            self.get_reg16(rm)
        } else {
            // Memory mode
            bus.memory_read_u16(addr)
        }
    }

    // Write 8-bit value to register or memory based on mod field
    fn write_rm8(&mut self, mode: u8, rm: u8, addr: usize, value: u8, bus: &mut Bus) {
        if mode == 0b11 {
            // Register mode
            self.set_reg8(rm, value);
        } else {
            // Memory mode
            bus.memory_write_u8(addr, value);
        }
    }

    // Write 16-bit value to register or memory based on mod field
    fn write_rm16(&mut self, mode: u8, rm: u8, addr: usize, value: u16, bus: &mut Bus) {
        if mode == 0b11 {
            // Register mode
            self.set_reg16(rm, value);
        } else {
            // Memory mode
            bus.memory_write_u16(addr, value);
        }
    }

    /// Push 16-bit value onto stack
    pub(in crate::cpu) fn push(&mut self, value: u16, bus: &mut Bus) {
        self.sp = self.sp.wrapping_sub(2);
        let addr = physical_address(self.ss, self.sp);
        bus.memory_write_u16(addr, value);
    }

    /// Pop 16-bit value from stack
    fn pop(&mut self, bus: &Bus) -> u16 {
        let addr = physical_address(self.ss, self.sp);
        let value = bus.memory_read_u16(addr);
        self.sp = self.sp.wrapping_add(2);
        value
    }

    // Get segment register value
    fn get_segreg(&self, reg: u8) -> u16 {
        match reg & 0x03 {
            0 => self.es,
            1 => self.cs,
            2 => self.ss,
            3 => self.ds,
            _ => unreachable!(),
        }
    }
}
