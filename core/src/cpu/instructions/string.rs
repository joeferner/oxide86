use crate::{
    cpu::{Cpu, cpu_flag, timing},
    memory_bus::MemoryBus, physical_address,
};

impl Cpu {
    /// LODS - Load String (opcodes AC-AD)
    /// AC: LODSB - Load byte from DS:SI into AL
    /// AD: LODSW - Load word from DS:SI into AX
    ///
    /// Loads data from DS:SI into AL/AX, then increments/decrements SI based on DF.
    /// Note: Segment override can apply to DS:SI
    pub(in crate::cpu) fn lods(&mut self, opcode: u8, memory_bus: &MemoryBus) {
        let is_word = opcode & 0x01 != 0;

        // Handle repeat prefix
        if self.repeat_prefix.is_some() {
            let count = self.cx;
            while self.cx != 0 {
                self.lods_once(is_word, memory_bus);
                self.cx = self.cx.wrapping_sub(1);
            }
            // REP LODS: 9 + 13*CX cycles
            self.last_instruction_cycles =
                timing::cycles::REP_LODS_BASE + (timing::cycles::REP_LODS_PER_ITER * count as u64);
        } else {
            self.lods_once(is_word, memory_bus);
            // LODS (no REP): 12 cycles
            self.last_instruction_cycles = timing::cycles::LODS;
        }
    }

    fn lods_once(&mut self, is_word: bool, memory_bus: &MemoryBus) {
        if is_word {
            // LODSW - Load word
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let addr = physical_address(src_seg, self.si);
            self.ax = memory_bus.read_u16(addr);

            // Update SI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.si = self.si.wrapping_sub(2);
            } else {
                self.si = self.si.wrapping_add(2);
            }
        } else {
            // LODSB - Load byte
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let addr = physical_address(src_seg, self.si);
            let value = memory_bus.read_u8(addr);
            self.ax = (self.ax & 0xFF00) | (value as u16);

            // Update SI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.si = self.si.wrapping_sub(1);
            } else {
                self.si = self.si.wrapping_add(1);
            }
        }
    }
}
