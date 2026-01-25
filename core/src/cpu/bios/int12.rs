use crate::{cpu::Cpu, memory::Memory};
use crate::memory::{BDA_START, BDA_MEMORY_SIZE};

impl Cpu {
    /// INT 0x12 - Get Memory Size
    /// Returns the amount of conventional memory (base memory below 1MB) in KB
    /// Input: None
    /// Output: AX = memory size in KB (typically 640)
    pub(super) fn handle_int12(&mut self, memory: &Memory) {
        // Read memory size from BDA at offset 0x13 (2 bytes)
        let mem_size = memory.read_word(BDA_START + BDA_MEMORY_SIZE);
        self.ax = mem_size;
    }
}
