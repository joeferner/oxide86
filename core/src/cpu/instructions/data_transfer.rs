use super::super::Cpu;
use crate::memory::Memory;

impl Cpu {
    /// MOV immediate to register (opcodes B0-BF)
    /// B0-B7: MOV reg8, imm8
    /// B8-BF: MOV reg16, imm16
    pub(in crate::cpu) fn mov_imm_to_reg(&mut self, opcode: u8, memory: &Memory) {
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
    pub(in crate::cpu) fn mov_reg_rm(&mut self, opcode: u8, memory: &Memory) {
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

    /// PUSH 16-bit register (opcodes 50-57)
    /// Push register onto stack
    pub(in crate::cpu) fn push_reg16(&mut self, opcode: u8, memory: &mut Memory) {
        let reg = opcode & 0x07;
        let value = self.get_reg16(reg);
        self.push(value, memory);
    }

    /// POP 16-bit register (opcodes 58-5F)
    /// Pop from stack to register
    pub(in crate::cpu) fn pop_reg16(&mut self, opcode: u8, memory: &mut Memory) {
        let reg = opcode & 0x07;
        let value = self.pop(memory);
        self.set_reg16(reg, value);
    }

    /// PUSH immediate (opcode 68: 16-bit, 6A: sign-extended 8-bit)
    pub(in crate::cpu) fn push_imm(&mut self, opcode: u8, memory: &mut Memory) {
        let value = if opcode == 0x68 {
            // PUSH imm16
            self.fetch_word(memory)
        } else {
            // PUSH imm8 (sign-extended to 16 bits)
            let imm8 = self.fetch_byte(memory);
            if imm8 & 0x80 != 0 {
                0xFF00 | (imm8 as u16)
            } else {
                imm8 as u16
            }
        };
        self.push(value, memory);
    }
}
