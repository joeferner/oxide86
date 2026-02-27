use crate::{cpu::{Cpu, cpu_flag, timing}, memory_bus::MemoryBus};

impl Cpu {
    /// TEST r/m and register (opcodes 84-85)
    /// 84: TEST r/m8, r8
    /// 85: TEST r/m16, r16
    pub(in crate::cpu) fn test_rm_reg(&mut self, opcode: u8, memory_bus: &mut MemoryBus) {
        let is_word = opcode & 0x01 != 0;

        let modrm = self.fetch_byte(memory_bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, memory_bus);

        if is_word {
            let src = self.get_reg16(reg);
            let dst = self.read_rm16(mode, rm, addr, memory_bus);
            let result = dst & src;

            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        } else {
            let src = self.get_reg8(reg);
            let dst = self.read_rm8(mode, rm, addr, memory_bus);
            let result = dst & src;

            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        }

        // TEST r/m, reg: 3 cycles (reg), 9+EA (mem)
        self.last_instruction_cycles = if mode == 0b11 {
            timing::cycles::TEST_REG_REG
        } else {
            timing::cycles::TEST_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }
}
