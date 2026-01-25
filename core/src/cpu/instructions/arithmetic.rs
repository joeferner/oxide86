use super::super::{Cpu, FLAG_AUXILIARY, FLAG_CARRY, FLAG_OVERFLOW};
use crate::memory::Memory;

impl Cpu {
    /// ADD r/m and register (opcodes 00-03)
    /// 00: ADD r/m8, r8
    /// 01: ADD r/m16, r16
    /// 02: ADD r8, r/m8
    /// 03: ADD r16, r/m16
    pub(in crate::cpu) fn add_rm_reg(&mut self, opcode: u8, memory: &Memory) {
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
    pub(in crate::cpu) fn add_imm_acc(&mut self, opcode: u8, memory: &Memory) {
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
    pub(in crate::cpu) fn arith_imm8_rm8(&mut self, memory: &Memory) {
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
                1 => {
                    // OR
                    let result = dst | imm;
                    self.set_reg8(rm, result);
                    self.set_flags_8(result);
                    self.set_flag(FLAG_CARRY, false);
                    self.set_flag(FLAG_OVERFLOW, false);
                }
                4 => {
                    // AND
                    let result = dst & imm;
                    self.set_reg8(rm, result);
                    self.set_flags_8(result);
                    self.set_flag(FLAG_CARRY, false);
                    self.set_flag(FLAG_OVERFLOW, false);
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
                6 => {
                    // XOR
                    let result = dst ^ imm;
                    self.set_reg8(rm, result);
                    self.set_flags_8(result);
                    self.set_flag(FLAG_CARRY, false);
                    self.set_flag(FLAG_OVERFLOW, false);
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
    pub(in crate::cpu) fn arith_imm16_rm(&mut self, memory: &Memory) {
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
    pub(in crate::cpu) fn arith_imm8_rm(&mut self, memory: &Memory) {
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
                1 => {
                    // OR
                    let result = dst | imm;
                    self.set_reg16(rm, result);
                    self.set_flags_16(result);
                    self.set_flag(FLAG_CARRY, false);
                    self.set_flag(FLAG_OVERFLOW, false);
                }
                4 => {
                    // AND
                    let result = dst & imm;
                    self.set_reg16(rm, result);
                    self.set_flags_16(result);
                    self.set_flag(FLAG_CARRY, false);
                    self.set_flag(FLAG_OVERFLOW, false);
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
                6 => {
                    // XOR
                    let result = dst ^ imm;
                    self.set_reg16(rm, result);
                    self.set_flags_16(result);
                    self.set_flag(FLAG_CARRY, false);
                    self.set_flag(FLAG_OVERFLOW, false);
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
    pub(in crate::cpu) fn sub_rm_reg(&mut self, opcode: u8, memory: &Memory) {
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
    pub(in crate::cpu) fn sub_imm_acc(&mut self, opcode: u8, memory: &Memory) {
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

    /// INC 16-bit register (opcodes 40-47)
    /// Increment register by 1 (does not affect carry flag)
    pub(in crate::cpu) fn inc_reg16(&mut self, opcode: u8) {
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
    pub(in crate::cpu) fn dec_reg16(&mut self, opcode: u8) {
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
    pub(in crate::cpu) fn inc_dec_rm(&mut self, opcode: u8, memory: &Memory) {
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
}
