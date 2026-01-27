use crate::memory::{BDA_MEMORY_SIZE, BDA_START};
use crate::{cpu::Cpu, memory::Memory};

impl Cpu {
    /// INT 0x12 - Get Memory Size
    /// Returns the amount of conventional memory (base memory below 1MB) in KB
    /// Input: None
    /// Output: AX = memory size in KB (typically 640)
    pub(super) fn handle_int12(&mut self, memory: &Memory) {
        // Read memory size from BDA at offset 0x13 (2 bytes)
        let mem_size = memory.read_u16(BDA_START + BDA_MEMORY_SIZE);
        self.ax = mem_size;
        log::info!("INT 12h: Returning memory size = {} KB", mem_size);
    }
}
