use crate::{
    bus::Bus,
    cpu::{Cpu, bios::bda::bda_get_memory_size},
};

impl Cpu {
    /// INT 0x12 - Get Memory Size
    /// Returns the amount of conventional memory (base memory below 1MB) in KB
    /// Input: None
    /// Output: AX = memory size in KB (typically 640)
    pub(in crate::cpu) fn handle_int12_get_memory_size(&mut self, bus: &mut Bus) {
        bus.increment_cycle_count(100);
        // Read memory size from BDA at offset 0x13 (2 bytes)
        let mem_size = bda_get_memory_size(bus);
        self.ax = mem_size;
        log::info!("INT 0x12: Returning memory size = {} KB", mem_size);
    }
}
