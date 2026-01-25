use super::{Cpu, FLAG_AUXILIARY, FLAG_CARRY, FLAG_OVERFLOW, FLAG_ZERO, FLAG_SIGN, FLAG_PARITY};
use crate::memory::Memory;

impl Cpu {
    /// HLT - Halt (opcode F4)
    /// Stops instruction execution until a hardware interrupt occurs
    pub(super) fn hlt(&mut self) {
        self.halted = true;
    }

    /// MOV immediate to register (opcodes B0-BF)
    /// B0-B7: MOV reg8, imm8
    /// B8-BF: MOV reg16, imm16
    pub(super) fn mov_imm_to_reg(&mut self, opcode: u8, memory: &Memory) {
        let reg = opcode & 0x07;
        let is_word = opcode & 0x08 != 0;

        if is_word {
            // 16-bit register
            let value = self.fetch_word(memory);
            self.set_reg16(reg, value);
        } else {
            // 8-bit register
            let value = self.fetch_byte(memory);
            self.set_reg8(reg, value);
        }
    }

    /// MOV register to/from r/m (opcodes 88-8B)
    /// 88: MOV r/m8, r8
    /// 89: MOV r/m16, r16
    /// 8A: MOV r8, r/m8
    /// 8B: MOV r16, r/m16
    pub(super) fn mov_reg_rm(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(memory);
        let reg = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;
        let mode = modrm >> 6;

        // For now, only handle register-to-register (mode = 11b)
        if mode == 0b11 {
            if is_word {
                // 16-bit register move
                if dir {
                    // MOV reg16, rm16
                    let value = self.get_reg16(rm);
                    self.set_reg16(reg, value);
                } else {
                    // MOV rm16, reg16
                    let value = self.get_reg16(reg);
                    self.set_reg16(rm, value);
                }
            } else {
                // 8-bit register move
                if dir {
                    // MOV reg8, rm8
                    let value = self.get_reg8(rm);
                    self.set_reg8(reg, value);
                } else {
                    // MOV rm8, reg8
                    let value = self.get_reg8(reg);
                    self.set_reg8(rm, value);
                }
            }
        } else {
            panic!("Memory addressing modes not yet implemented");
        }
    }

    /// ADD r/m and register (opcodes 00-03)
    /// 00: ADD r/m8, r8
    /// 01: ADD r/m16, r16
    /// 02: ADD r8, r/m8
    /// 03: ADD r16, r/m16
    pub(super) fn add_rm_reg(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(memory);
        let reg = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;
        let mode = modrm >> 6;

        // For now, only handle register-to-register (mode = 11b)
        if mode == 0b11 {
            if is_word {
                // 16-bit register add
                let src = if dir { self.get_reg16(rm) } else { self.get_reg16(reg) };
                let dst = if dir { self.get_reg16(reg) } else { self.get_reg16(rm) };

                let (result, carry) = dst.overflowing_add(src);
                let overflow = ((dst ^ result) & (src ^ result) & 0x8000) != 0;

                if dir {
                    self.set_reg16(reg, result);
                } else {
                    self.set_reg16(rm, result);
                }

                self.set_flags_16(result);
                self.set_flag(FLAG_CARRY, carry);
                self.set_flag(FLAG_OVERFLOW, overflow);
            } else {
                // 8-bit register add
                let src = if dir { self.get_reg8(rm) } else { self.get_reg8(reg) };
                let dst = if dir { self.get_reg8(reg) } else { self.get_reg8(rm) };

                let (result, carry) = dst.overflowing_add(src);
                let overflow = ((dst ^ result) & (src ^ result) & 0x80) != 0;
                let aux_carry = ((dst & 0x0F) + (src & 0x0F)) > 0x0F;

                if dir {
                    self.set_reg8(reg, result);
                } else {
                    self.set_reg8(rm, result);
                }

                self.set_flags_8(result);
                self.set_flag(FLAG_CARRY, carry);
                self.set_flag(FLAG_OVERFLOW, overflow);
                self.set_flag(FLAG_AUXILIARY, aux_carry);
            }
        } else {
            panic!("Memory addressing modes not yet implemented");
        }
    }

    /// ADD immediate to accumulator (opcodes 04-05)
    /// 04: ADD AL, imm8
    /// 05: ADD AX, imm16
    pub(super) fn add_imm_acc(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            // ADD AX, imm16
            let imm = self.fetch_word(memory);
            let (result, carry) = self.ax.overflowing_add(imm);
            let overflow = ((self.ax ^ result) & (imm ^ result) & 0x8000) != 0;

            self.ax = result;
            self.set_flags_16(result);
            self.set_flag(FLAG_CARRY, carry);
            self.set_flag(FLAG_OVERFLOW, overflow);
        } else {
            // ADD AL, imm8
            let imm = self.fetch_byte(memory);
            let al = (self.ax & 0xFF) as u8;
            let (result, carry) = al.overflowing_add(imm);
            let overflow = ((al ^ result) & (imm ^ result) & 0x80) != 0;
            let aux_carry = ((al & 0x0F) + (imm & 0x0F)) > 0x0F;

            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(FLAG_CARRY, carry);
            self.set_flag(FLAG_OVERFLOW, overflow);
            self.set_flag(FLAG_AUXILIARY, aux_carry);
        }
    }

    /// Arithmetic with immediate to r/m (opcode 0x80)
    /// 80: Immediate Group 1 - ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m8, imm8
    pub(super) fn arith_imm8_rm8(&mut self, memory: &Memory) {
        let modrm = self.fetch_byte(memory);
        let operation = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;
        let mode = modrm >> 6;

        // For now, only handle register mode (mode = 11b)
        if mode == 0b11 {
            let imm = self.fetch_byte(memory);
            let dst = self.get_reg8(rm);

            match operation {
                0 => {
                    // ADD
                    let (result, carry) = dst.overflowing_add(imm);
                    let overflow = ((dst ^ result) & (imm ^ result) & 0x80) != 0;
                    let aux_carry = ((dst & 0x0F) + (imm & 0x0F)) > 0x0F;
                    self.set_reg8(rm, result);
                    self.set_flags_8(result);
                    self.set_flag(FLAG_CARRY, carry);
                    self.set_flag(FLAG_OVERFLOW, overflow);
                    self.set_flag(FLAG_AUXILIARY, aux_carry);
                }
                5 => {
                    // SUB
                    let (result, carry) = dst.overflowing_sub(imm);
                    let overflow = ((dst ^ imm) & (dst ^ result) & 0x80) != 0;
                    let aux_carry = (dst & 0x0F) < (imm & 0x0F);
                    self.set_reg8(rm, result);
                    self.set_flags_8(result);
                    self.set_flag(FLAG_CARRY, carry);
                    self.set_flag(FLAG_OVERFLOW, overflow);
                    self.set_flag(FLAG_AUXILIARY, aux_carry);
                }
                7 => {
                    // CMP (like SUB but doesn't store result)
                    let (result, carry) = dst.overflowing_sub(imm);
                    let overflow = ((dst ^ imm) & (dst ^ result) & 0x80) != 0;
                    let aux_carry = (dst & 0x0F) < (imm & 0x0F);
                    self.set_flags_8(result);
                    self.set_flag(FLAG_CARRY, carry);
                    self.set_flag(FLAG_OVERFLOW, overflow);
                    self.set_flag(FLAG_AUXILIARY, aux_carry);
                }
                _ => panic!("Unimplemented arithmetic operation: {}", operation),
            }
        } else {
            panic!("Memory addressing modes not yet implemented");
        }
    }

    /// Arithmetic with immediate to r/m (opcode 0x81)
    /// 81: Immediate Group 1 - ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m16, imm16
    pub(super) fn arith_imm16_rm(&mut self, memory: &Memory) {
        let modrm = self.fetch_byte(memory);
        let operation = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;
        let mode = modrm >> 6;

        // For now, only handle register mode (mode = 11b)
        if mode == 0b11 {
            let imm = self.fetch_word(memory);
            let dst = self.get_reg16(rm);

            match operation {
                0 => {
                    // ADD
                    let (result, carry) = dst.overflowing_add(imm);
                    let overflow = ((dst ^ result) & (imm ^ result) & 0x8000) != 0;
                    self.set_reg16(rm, result);
                    self.set_flags_16(result);
                    self.set_flag(FLAG_CARRY, carry);
                    self.set_flag(FLAG_OVERFLOW, overflow);
                }
                _ => panic!("Unimplemented arithmetic operation: {}", operation),
            }
        } else {
            panic!("Memory addressing modes not yet implemented");
        }
    }

    /// Arithmetic with sign-extended immediate to r/m (opcode 0x83)
    /// 83: Immediate Group 1 - ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m16, imm8 (sign-extended)
    pub(super) fn arith_imm8_rm(&mut self, memory: &Memory) {
        let modrm = self.fetch_byte(memory);
        let operation = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;
        let mode = modrm >> 6;

        // For now, only handle register mode (mode = 11b)
        if mode == 0b11 {
            let imm8 = self.fetch_byte(memory);
            // Sign-extend the 8-bit immediate to 16 bits
            let imm = if imm8 & 0x80 != 0 {
                0xFF00 | (imm8 as u16)
            } else {
                imm8 as u16
            };
            let dst = self.get_reg16(rm);

            match operation {
                0 => {
                    // ADD
                    let (result, carry) = dst.overflowing_add(imm);
                    let overflow = ((dst ^ result) & (imm ^ result) & 0x8000) != 0;
                    self.set_reg16(rm, result);
                    self.set_flags_16(result);
                    self.set_flag(FLAG_CARRY, carry);
                    self.set_flag(FLAG_OVERFLOW, overflow);
                }
                5 => {
                    // SUB
                    let (result, carry) = dst.overflowing_sub(imm);
                    let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000) != 0;
                    self.set_reg16(rm, result);
                    self.set_flags_16(result);
                    self.set_flag(FLAG_CARRY, carry);
                    self.set_flag(FLAG_OVERFLOW, overflow);
                }
                7 => {
                    // CMP (like SUB but doesn't store result)
                    let (result, carry) = dst.overflowing_sub(imm);
                    let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000) != 0;
                    self.set_flags_16(result);
                    self.set_flag(FLAG_CARRY, carry);
                    self.set_flag(FLAG_OVERFLOW, overflow);
                }
                _ => panic!("Unimplemented arithmetic operation: {}", operation),
            }
        } else {
            panic!("Memory addressing modes not yet implemented");
        }
    }

    /// SUB r/m and register (opcodes 28-2B)
    /// 28: SUB r/m8, r8
    /// 29: SUB r/m16, r16
    /// 2A: SUB r8, r/m8
    /// 2B: SUB r16, r/m16
    pub(super) fn sub_rm_reg(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(memory);
        let reg = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;
        let mode = modrm >> 6;

        // For now, only handle register-to-register (mode = 11b)
        if mode == 0b11 {
            if is_word {
                // 16-bit register sub
                let src = if dir { self.get_reg16(rm) } else { self.get_reg16(reg) };
                let dst = if dir { self.get_reg16(reg) } else { self.get_reg16(rm) };

                let (result, carry) = dst.overflowing_sub(src);
                let overflow = ((dst ^ src) & (dst ^ result) & 0x8000) != 0;

                if dir {
                    self.set_reg16(reg, result);
                } else {
                    self.set_reg16(rm, result);
                }

                self.set_flags_16(result);
                self.set_flag(FLAG_CARRY, carry);
                self.set_flag(FLAG_OVERFLOW, overflow);
            } else {
                // 8-bit register sub
                let src = if dir { self.get_reg8(rm) } else { self.get_reg8(reg) };
                let dst = if dir { self.get_reg8(reg) } else { self.get_reg8(rm) };

                let (result, carry) = dst.overflowing_sub(src);
                let overflow = ((dst ^ src) & (dst ^ result) & 0x80) != 0;
                let aux_carry = (dst & 0x0F) < (src & 0x0F);

                if dir {
                    self.set_reg8(reg, result);
                } else {
                    self.set_reg8(rm, result);
                }

                self.set_flags_8(result);
                self.set_flag(FLAG_CARRY, carry);
                self.set_flag(FLAG_OVERFLOW, overflow);
                self.set_flag(FLAG_AUXILIARY, aux_carry);
            }
        } else {
            panic!("Memory addressing modes not yet implemented");
        }
    }

    /// SUB immediate to accumulator (opcodes 2C-2D)
    /// 2C: SUB AL, imm8
    /// 2D: SUB AX, imm16
    pub(super) fn sub_imm_acc(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            // SUB AX, imm16
            let imm = self.fetch_word(memory);
            let (result, carry) = self.ax.overflowing_sub(imm);
            let overflow = ((self.ax ^ imm) & (self.ax ^ result) & 0x8000) != 0;

            self.ax = result;
            self.set_flags_16(result);
            self.set_flag(FLAG_CARRY, carry);
            self.set_flag(FLAG_OVERFLOW, overflow);
        } else {
            // SUB AL, imm8
            let imm = self.fetch_byte(memory);
            let al = (self.ax & 0xFF) as u8;
            let (result, carry) = al.overflowing_sub(imm);
            let overflow = ((al ^ imm) & (al ^ result) & 0x80) != 0;
            let aux_carry = (al & 0x0F) < (imm & 0x0F);

            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(FLAG_CARRY, carry);
            self.set_flag(FLAG_OVERFLOW, overflow);
            self.set_flag(FLAG_AUXILIARY, aux_carry);
        }
    }

    /// CMP r/m and register (opcodes 38-3B)
    /// 38: CMP r/m8, r8
    /// 39: CMP r/m16, r16
    /// 3A: CMP r8, r/m8
    /// 3B: CMP r16, r/m16
    pub(super) fn cmp_rm_reg(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(memory);
        let reg = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;
        let mode = modrm >> 6;

        // For now, only handle register-to-register (mode = 11b)
        if mode == 0b11 {
            if is_word {
                // 16-bit register cmp
                let src = if dir { self.get_reg16(rm) } else { self.get_reg16(reg) };
                let dst = if dir { self.get_reg16(reg) } else { self.get_reg16(rm) };

                let (result, carry) = dst.overflowing_sub(src);
                let overflow = ((dst ^ src) & (dst ^ result) & 0x8000) != 0;

                self.set_flags_16(result);
                self.set_flag(FLAG_CARRY, carry);
                self.set_flag(FLAG_OVERFLOW, overflow);
            } else {
                // 8-bit register cmp
                let src = if dir { self.get_reg8(rm) } else { self.get_reg8(reg) };
                let dst = if dir { self.get_reg8(reg) } else { self.get_reg8(rm) };

                let (result, carry) = dst.overflowing_sub(src);
                let overflow = ((dst ^ src) & (dst ^ result) & 0x80) != 0;
                let aux_carry = (dst & 0x0F) < (src & 0x0F);

                self.set_flags_8(result);
                self.set_flag(FLAG_CARRY, carry);
                self.set_flag(FLAG_OVERFLOW, overflow);
                self.set_flag(FLAG_AUXILIARY, aux_carry);
            }
        } else {
            panic!("Memory addressing modes not yet implemented");
        }
    }

    /// CMP immediate to accumulator (opcodes 3C-3D)
    /// 3C: CMP AL, imm8
    /// 3D: CMP AX, imm16
    pub(super) fn cmp_imm_acc(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            // CMP AX, imm16
            let imm = self.fetch_word(memory);
            let (result, carry) = self.ax.overflowing_sub(imm);
            let overflow = ((self.ax ^ imm) & (self.ax ^ result) & 0x8000) != 0;

            self.set_flags_16(result);
            self.set_flag(FLAG_CARRY, carry);
            self.set_flag(FLAG_OVERFLOW, overflow);
        } else {
            // CMP AL, imm8
            let imm = self.fetch_byte(memory);
            let al = (self.ax & 0xFF) as u8;
            let (result, carry) = al.overflowing_sub(imm);
            let overflow = ((al ^ imm) & (al ^ result) & 0x80) != 0;
            let aux_carry = (al & 0x0F) < (imm & 0x0F);

            self.set_flags_8(result);
            self.set_flag(FLAG_CARRY, carry);
            self.set_flag(FLAG_OVERFLOW, overflow);
            self.set_flag(FLAG_AUXILIARY, aux_carry);
        }
    }

    /// JMP short relative (opcode EB)
    /// Jump to IP + signed 8-bit displacement
    pub(super) fn jmp_short(&mut self, memory: &Memory) {
        let offset = self.fetch_byte(memory) as i8;
        self.ip = self.ip.wrapping_add(offset as i16 as u16);
    }

    /// JMP near relative (opcode E9)
    /// Jump to IP + signed 16-bit displacement
    pub(super) fn jmp_near(&mut self, memory: &Memory) {
        let offset = self.fetch_word(memory) as i16;
        self.ip = self.ip.wrapping_add(offset as u16);
    }

    /// INC 16-bit register (opcodes 40-47)
    /// Increment register by 1 (does not affect carry flag)
    pub(super) fn inc_reg16(&mut self, opcode: u8) {
        let reg = opcode & 0x07;
        let value = self.get_reg16(reg);
        let result = value.wrapping_add(1);

        self.set_reg16(reg, result);
        self.set_flags_16(result);
        // INC does not affect the carry flag
        let overflow = value == 0x7FFF; // Overflow when going from max positive to negative
        self.set_flag(FLAG_OVERFLOW, overflow);
        let aux_carry = (value & 0x0F) == 0x0F;
        self.set_flag(FLAG_AUXILIARY, aux_carry);
    }

    /// DEC 16-bit register (opcodes 48-4F)
    /// Decrement register by 1 (does not affect carry flag)
    pub(super) fn dec_reg16(&mut self, opcode: u8) {
        let reg = opcode & 0x07;
        let value = self.get_reg16(reg);
        let result = value.wrapping_sub(1);

        self.set_reg16(reg, result);
        self.set_flags_16(result);
        // DEC does not affect the carry flag
        let overflow = value == 0x8000; // Overflow when going from min negative to positive
        self.set_flag(FLAG_OVERFLOW, overflow);
        let aux_carry = (value & 0x0F) == 0;
        self.set_flag(FLAG_AUXILIARY, aux_carry);
    }

    /// INC/DEC r/m (opcode 0xFE for 8-bit, 0xFF for 16-bit)
    /// FE /0: INC r/m8
    /// FE /1: DEC r/m8
    /// FF /0: INC r/m16
    /// FF /1: DEC r/m16
    pub(super) fn inc_dec_rm(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;
        let modrm = self.fetch_byte(memory);
        let operation = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;
        let mode = modrm >> 6;

        // For now, only handle register mode (mode = 11b)
        if mode == 0b11 {
            match operation {
                0 => {
                    // INC
                    if is_word {
                        let value = self.get_reg16(rm);
                        let result = value.wrapping_add(1);
                        self.set_reg16(rm, result);
                        self.set_flags_16(result);
                        let overflow = value == 0x7FFF;
                        self.set_flag(FLAG_OVERFLOW, overflow);
                        let aux_carry = (value & 0x0F) == 0x0F;
                        self.set_flag(FLAG_AUXILIARY, aux_carry);
                    } else {
                        let value = self.get_reg8(rm);
                        let result = value.wrapping_add(1);
                        self.set_reg8(rm, result);
                        self.set_flags_8(result);
                        let overflow = value == 0x7F;
                        self.set_flag(FLAG_OVERFLOW, overflow);
                        let aux_carry = (value & 0x0F) == 0x0F;
                        self.set_flag(FLAG_AUXILIARY, aux_carry);
                    }
                }
                1 => {
                    // DEC
                    if is_word {
                        let value = self.get_reg16(rm);
                        let result = value.wrapping_sub(1);
                        self.set_reg16(rm, result);
                        self.set_flags_16(result);
                        let overflow = value == 0x8000;
                        self.set_flag(FLAG_OVERFLOW, overflow);
                        let aux_carry = (value & 0x0F) == 0;
                        self.set_flag(FLAG_AUXILIARY, aux_carry);
                    } else {
                        let value = self.get_reg8(rm);
                        let result = value.wrapping_sub(1);
                        self.set_reg8(rm, result);
                        self.set_flags_8(result);
                        let overflow = value == 0x80;
                        self.set_flag(FLAG_OVERFLOW, overflow);
                        let aux_carry = (value & 0x0F) == 0;
                        self.set_flag(FLAG_AUXILIARY, aux_carry);
                    }
                }
                _ => panic!("Invalid INC/DEC operation: {}", operation),
            }
        } else {
            panic!("Memory addressing modes not yet implemented");
        }
    }

    /// Conditional jumps - short relative (opcodes 70-7F)
    /// Jump to IP + signed 8-bit displacement if condition is met
    pub(super) fn jmp_conditional(&mut self, opcode: u8, memory: &Memory) {
        let offset = self.fetch_byte(memory) as i8;

        let condition = match opcode {
            0x70 => self.get_flag(FLAG_OVERFLOW),                    // JO - Jump if overflow
            0x71 => !self.get_flag(FLAG_OVERFLOW),                   // JNO - Jump if not overflow
            0x72 => self.get_flag(FLAG_CARRY),                       // JB/JC/JNAE - Jump if below/carry
            0x73 => !self.get_flag(FLAG_CARRY),                      // JAE/JNB/JNC - Jump if above or equal/not below/not carry
            0x74 => self.get_flag(FLAG_ZERO),                        // JE/JZ - Jump if equal/zero
            0x75 => !self.get_flag(FLAG_ZERO),                       // JNE/JNZ - Jump if not equal/not zero
            0x76 => self.get_flag(FLAG_CARRY) || self.get_flag(FLAG_ZERO),  // JBE/JNA - Jump if below or equal/not above
            0x77 => !self.get_flag(FLAG_CARRY) && !self.get_flag(FLAG_ZERO), // JA/JNBE - Jump if above/not below or equal
            0x78 => self.get_flag(FLAG_SIGN),                        // JS - Jump if sign
            0x79 => !self.get_flag(FLAG_SIGN),                       // JNS - Jump if not sign
            0x7A => self.get_flag(FLAG_PARITY),                      // JP/JPE - Jump if parity/parity even
            0x7B => !self.get_flag(FLAG_PARITY),                     // JNP/JPO - Jump if not parity/parity odd
            0x7C => self.get_flag(FLAG_SIGN) != self.get_flag(FLAG_OVERFLOW),  // JL/JNGE - Jump if less/not greater or equal
            0x7D => self.get_flag(FLAG_SIGN) == self.get_flag(FLAG_OVERFLOW),  // JGE/JNL - Jump if greater or equal/not less
            0x7E => self.get_flag(FLAG_ZERO) || (self.get_flag(FLAG_SIGN) != self.get_flag(FLAG_OVERFLOW)),  // JLE/JNG - Jump if less or equal/not greater
            0x7F => !self.get_flag(FLAG_ZERO) && (self.get_flag(FLAG_SIGN) == self.get_flag(FLAG_OVERFLOW)), // JG/JNLE - Jump if greater/not less or equal
            _ => unreachable!(),
        };

        if condition {
            self.ip = self.ip.wrapping_add(offset as i16 as u16);
        }
    }
}
