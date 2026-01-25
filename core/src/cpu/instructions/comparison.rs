use super::super::{Cpu, FLAG_AUXILIARY, FLAG_CARRY, FLAG_OVERFLOW};
use crate::memory::Memory;

impl Cpu {
    /// CMP r/m and register (opcodes 38-3B)
    /// 38: CMP r/m8, r8
    /// 39: CMP r/m16, r16
    /// 3A: CMP r8, r/m8
    /// 3B: CMP r16, r/m16
    pub(in crate::cpu) fn cmp_rm_reg(&mut self, opcode: u8, memory: &Memory) {
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
    pub(in crate::cpu) fn cmp_imm_acc(&mut self, opcode: u8, memory: &Memory) {
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
}
