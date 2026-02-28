use crate::{
    cpu::{IVT_END, IVT_ENTRY_SIZE, IVT_START, bios::bda::bda_reset},
    memory_bus::MemoryBus,
};

pub mod bda;
pub mod int10_video_services;
pub mod int13_disk_services;
pub mod int21_dos_services;
pub mod int11_get_equipment_list;
pub mod int17_printer_services;

// BIOS code segment
pub const BIOS_CODE_SEGMENT: u16 = 0xF000;

pub fn bios_reset(memory_bus: &mut MemoryBus) {
    bios_interrupt_handlers_reset(memory_bus);
    bda_reset(memory_bus);
}

fn bios_interrupt_handlers_reset(memory_bus: &mut MemoryBus) {
    for addr in (IVT_START..=IVT_END).step_by(IVT_ENTRY_SIZE) {
        memory_bus.write_u32(addr, ((BIOS_CODE_SEGMENT as u32) << 16) | addr as u32);
    }
}
