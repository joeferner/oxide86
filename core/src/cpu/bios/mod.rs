use crate::{
    bus::Bus,
    cpu::{IVT_END, IVT_ENTRY_SIZE, IVT_START, bios::bda::bda_reset},
};

pub mod bda;
pub mod int09_keyboard_hardware_interrupt;
pub mod int10_video_services;
pub mod int11_get_equipment_list;
pub mod int12_get_memory_size;
pub mod int13_disk_services;
pub mod int16_keyboard_services;
pub mod int17_printer_services;
pub mod int1a_time_services;
pub mod int21_dos_services;

// BIOS code segment
pub const BIOS_CODE_SEGMENT: u16 = 0xF000;

pub fn bios_reset(bus: &mut Bus) {
    bios_interrupt_handlers_reset(bus);
    bda_reset(bus);
}

fn bios_interrupt_handlers_reset(bus: &mut Bus) {
    for addr in (IVT_START..=IVT_END).step_by(IVT_ENTRY_SIZE) {
        let irq = addr / IVT_ENTRY_SIZE;
        bus.memory_write_u32(addr, ((BIOS_CODE_SEGMENT as u32) << 16) | irq as u32);
    }
}
