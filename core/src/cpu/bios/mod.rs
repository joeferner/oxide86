use crate::{
    bus::Bus,
    cpu::{IVT_END, IVT_ENTRY_SIZE, IVT_START, bios::bda::bda_reset},
    video::font::CGA_FONT_8X8_DATA,
};

pub mod bda;
pub mod int08_timer_interrupt;
pub mod int09_keyboard_hardware_interrupt;
pub mod int10_video_services;
pub mod int11_get_equipment_list;
pub mod int12_get_memory_size;
pub mod int13_disk_services;
pub mod int14_serial_port_services;
pub mod int15_miscellaneous;
pub mod int16_keyboard_services;
pub mod int17_printer_services;
pub mod int1a_time_services;
pub mod int21_dos_services;
pub mod int43_font_services;
pub mod int74_ps2_mouse_interrupt;

// BIOS code segment
pub const BIOS_CODE_SEGMENT: u16 = 0xF000;

// INT 15h AH=C0h system descriptor table location (in BIOS ROM area)
pub const INT15_SYSTEM_CONFIG_SEGMENT: u16 = 0xF000;
pub const INT15_SYSTEM_CONFIG_OFFSET: u16 = 0xE000;

/// Physical address of the IBM 8x8 CGA font in the BIOS ROM area (F000:FA6E).
/// This is the classic IBM PC BIOS location used by games to copy glyph data.
pub const BIOS_CGA_FONT_ADDR: usize = 0xFFA6E;

pub(crate) fn bios_reset(bus: &mut Bus) {
    bios_interrupt_handlers_reset(bus);
    bda_reset(bus);
    bios_int15_system_config_reset(bus);
    bios_font_reset(bus);
}

/// Writes the IBM 8x8 CGA font into the BIOS ROM area at F000:FA6E and sets
/// INT 43h / INT 1Fh vectors to point to it, matching real IBM PC BIOS behaviour.
fn bios_font_reset(bus: &mut Bus) {
    for (i, &byte) in CGA_FONT_8X8_DATA.iter().enumerate() {
        bus.memory_write_u8(BIOS_CGA_FONT_ADDR + i, byte);
    }
    // INT 43h (0x010C): full 256-char font table at F000:FA6E
    bus.memory_write_u16(0x010C, 0xFA6E);
    bus.memory_write_u16(0x010E, 0xF000);
    // INT 1Fh (0x007C): chars 0x80-0xFF start at F000:FA6E + 0x400
    bus.memory_write_u16(0x007C, 0xFE6E);
    bus.memory_write_u16(0x007E, 0xF000);
}

/// Writes the INT 15h AH=C0h system descriptor table into the BIOS ROM area at F000:E000.
/// This mirrors real BIOS behavior where the table is static data in ROM, not built on demand.
pub(crate) fn bios_int15_system_config_reset(bus: &mut Bus) {
    let table: [u8; 10] = [
        0x08, 0x00, // Length: 8 bytes (not including length field)
        0xFF, // Model byte: 0xFF = PC
        0x00, // Submodel: 0 = PC
        0x01, // BIOS revision: 1
        0x00, // Feature byte 1: no special features
        0x00, // Feature byte 2
        0x00, // Feature byte 3
        0x00, // Feature byte 4
        0x00, // Feature byte 5
    ];

    let physical_addr =
        ((INT15_SYSTEM_CONFIG_SEGMENT as usize) << 4) + INT15_SYSTEM_CONFIG_OFFSET as usize;
    for (i, &byte) in table.iter().enumerate() {
        bus.memory_write_u8(physical_addr + i, byte);
    }
}

fn bios_interrupt_handlers_reset(bus: &mut Bus) {
    for addr in (IVT_START..=IVT_END).step_by(IVT_ENTRY_SIZE) {
        let irq = addr / IVT_ENTRY_SIZE;
        bus.memory_write_u32(addr, ((BIOS_CODE_SEGMENT as u32) << 16) | irq as u32);
    }
}
