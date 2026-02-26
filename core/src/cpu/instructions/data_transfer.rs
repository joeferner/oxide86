use crate::{cpu::{Cpu, timing}, memory::MemoryBus};

impl Cpu {
    /// MOV immediate to register (opcodes B0-BF)
    /// B0-B7: MOV reg8, imm8
    /// B8-BF: MOV reg16, imm16
    pub(in crate::cpu) fn mov_imm_to_reg(&mut self, opcode: u8, memory_bus: &MemoryBus) {
        let reg = opcode & 0x07;
        let is_word = opcode & 0x08 != 0;

        if is_word {
            // 16-bit register
            let value = self.fetch_word(memory_bus);
            self.set_reg16(reg, value);
        } else {
            // 8-bit register
            let value = self.fetch_byte(memory_bus);
            self.set_reg8(reg, value);
        }

        // MOV immediate to register: 4 cycles
        self.last_instruction_cycles = timing::cycles::MOV_IMM_REG;
    }
}
