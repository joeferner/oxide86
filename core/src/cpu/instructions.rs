use super::Cpu;
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
}
