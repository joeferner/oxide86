use super::{Cpu, FLAG_AUXILIARY, FLAG_CARRY, FLAG_OVERFLOW};
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
                _ => panic!("Unimplemented arithmetic operation: {}", operation),
            }
        } else {
            panic!("Memory addressing modes not yet implemented");
        }
    }
}
