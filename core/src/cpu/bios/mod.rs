use crate::{
    bus::Bus,
    cpu::{IVT_END, IVT_ENTRY_SIZE, IVT_START, bios::bda::bda_reset},
    video::{
        VideoCardType,
        font::{CGA_FONT_8X8_DATA, EGA_FONT_8X14_DATA, VGA_FONT_8X16_DATA},
    },
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
pub mod int70_rtc_alarm_interrupt;
pub mod int74_ps2_mouse_interrupt;

// BIOS code segment
pub const BIOS_CODE_SEGMENT: u16 = 0xF000;

// INT 15h AH=C0h system descriptor table location (in BIOS ROM area)
pub const INT15_SYSTEM_CONFIG_SEGMENT: u16 = 0xF000;
pub const INT15_SYSTEM_CONFIG_OFFSET: u16 = 0xE000;

/// Physical address of the IBM 8x8 CGA font in the BIOS ROM area (F000:C000).
/// Placed at F000:C000 so all 256 characters (256*8 = 2048 bytes) fit within the
/// 16-bit offset range without wrapping (C000 + 0x800 = C800 < 0x10000). The
/// classic IBM BIOS location F000:FA6E causes characters above ~0xB2 to overflow
/// the 16-bit offset, producing wrong physical addresses when programs index into
/// the font table via ES:BP + char*8.
pub const BIOS_CGA_FONT_ADDR: usize = 0xFC000;
/// Segment offset of the 8x8 font within F000.
pub const BIOS_CGA_FONT_OFFSET: u16 = 0xC000;

/// Physical address of the IBM 8x14 EGA font in the BIOS ROM area (F000:C800).
/// Placed immediately after the 8x8 font (256*8 = 2048 = 0x800 bytes).
/// The 8x14 font occupies 256*14 = 3584 = 0xE00 bytes (C800..D5FF).
pub const BIOS_EGA_FONT_ADDR: usize = 0xFC800;
/// Segment offset of the 8x14 EGA font within F000.
pub const BIOS_EGA_FONT_OFFSET: u16 = 0xC800;

/// Physical address of the IBM 8x16 VGA font in the BIOS ROM area (F000:B000).
/// Placed before the 8x8 font. The 8x16 font occupies 256*16 = 4096 = 0x1000 bytes (B000..BFFF).
pub const BIOS_VGA_FONT_ADDR: usize = 0xFB000;
/// Segment offset of the 8x16 VGA font within F000.
pub const BIOS_VGA_FONT_OFFSET: u16 = 0xB000;

/// Physical address of the VGA static functionality table in BIOS ROM (F000:D600).
/// Placed after the EGA 8x14 font (C800..D5FF). Only 16 bytes.
/// Used by INT 10h/AH=1Bh to report supported video modes and capabilities.
pub const VGA_STATIC_FUNC_TABLE_ADDR: usize = 0xFD600;
/// Segment offset of the VGA static functionality table within F000.
pub const VGA_STATIC_FUNC_TABLE_OFFSET: u16 = 0xD600;

pub(crate) fn bios_reset(bus: &mut Bus) {
    bios_interrupt_handlers_reset(bus);
    bda_reset(bus);
    bios_int15_system_config_reset(bus);
    bios_font_reset(bus);
    bios_dma_reset(bus);
    if bus.video_card().card_type() == VideoCardType::VGA {
        bios_vga_static_func_table_reset(bus);
    }
}

/// Programs DMA1 channel 0 for memory refresh, matching real IBM PC BIOS POST behaviour.
///
/// Channel 0 is the memory-refresh channel; its DREQ line is permanently
/// asserted by hardware.  The BIOS initialises it so that diagnostic programs
/// (e.g. CheckIt) can verify the DMA controller is alive by polling the
/// current-address / current-count registers and expecting them to advance.
///
/// Setup: single transfer, address increment, auto-init, verify mode.
/// Count = 0xFFFF so the channel cycles continuously without masking itself.
fn bios_dma_reset(bus: &mut Bus) {
    // Master clear — resets flip-flop, command, status, and masks all channels.
    bus.io_write_u8(0x000D, 0x00);
    // Clear byte-pointer flip-flop so subsequent writes start at the low byte.
    bus.io_write_u8(0x000C, 0x00);
    // Channel 0 base/current address = 0x0000.
    bus.io_write_u8(0x0000, 0x00); // low byte
    bus.io_write_u8(0x0000, 0x00); // high byte
    // Channel 0 base/current count = 0xFFFF (65 536 bytes per refresh cycle).
    bus.io_write_u8(0x0001, 0xFF); // low byte
    bus.io_write_u8(0x0001, 0xFF); // high byte
    // Mode: single transfer | address increment | auto-init | verify | channel 0.
    bus.io_write_u8(0x000B, 0x58);
    // Channel 1: single-cycle | increment | no-auto-init | read (mem→device) | ch1.
    // IBM AT BIOS pre-initialises ch1 so drivers (e.g. SB16) only need to program
    // address/count/page before asserting DREQ, without touching the mode register.
    bus.io_write_u8(0x000B, 0x49);
    // Channel 3: single-cycle | increment | no-auto-init | read | ch3 (generic default).
    bus.io_write_u8(0x000B, 0x4B);
    // Unmask all DMA1 channels (matching IBM AT BIOS POST behaviour).
    bus.io_write_u8(0x000E, 0x00);
}

/// Writes the IBM 8x8 CGA font and IBM 8x14 EGA font into the BIOS ROM area and
/// sets INT 43h / INT 1Fh vectors to point to the 8x8 font (default for text/CGA modes).
pub(crate) fn bios_font_reset(bus: &mut Bus) {
    // 8x8 CGA font at F000:C000
    for (i, &byte) in CGA_FONT_8X8_DATA.iter().enumerate() {
        bus.memory_write_u8(BIOS_CGA_FONT_ADDR + i, byte);
    }
    // 8x14 EGA font at F000:C800
    for (i, &byte) in EGA_FONT_8X14_DATA.iter().enumerate() {
        bus.memory_write_u8(BIOS_EGA_FONT_ADDR + i, byte);
    }
    // 8x16 VGA font at F000:B000
    for (i, &byte) in VGA_FONT_8X16_DATA.iter().enumerate() {
        bus.memory_write_u8(BIOS_VGA_FONT_ADDR + i, byte);
    }
    // INT 43h (0x010C): full 256-char font table, default to 8x8 at F000:C000
    bus.memory_write_u16(0x010C, BIOS_CGA_FONT_OFFSET);
    bus.memory_write_u16(0x010E, 0xF000);
    // INT 1Fh (0x007C): chars 0x80-0xFF start at F000:C000 + 0x400
    bus.memory_write_u16(0x007C, BIOS_CGA_FONT_OFFSET + 0x400);
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

/// Writes the VGA static functionality table into BIOS ROM at F000:D600.
/// This table is referenced by INT 10h/AH=1Bh (offset 00h-03h of the response buffer)
/// and describes which video modes and features the VGA BIOS supports.
/// Programs like CheckIt read this table during initialization to determine
/// which video modes are available for testing.
fn bios_vga_static_func_table_reset(bus: &mut Bus) {
    let table: [u8; 16] = [
        // Byte 00h: Supported modes 00h-07h (bit N = mode N)
        0xFF, // All standard text and CGA modes
        // Byte 01h: Supported modes 08h-0Fh (bit N = mode N+8)
        0xE0, // Modes 0Dh, 0Eh, 0Fh (EGA modes; skip PCjr modes 08h-0Ch)
        // Byte 02h: Supported modes 10h-13h (bits 0-3)
        0x0F, // Modes 10h, 11h, 12h, 13h all supported
        // Bytes 03h-06h: Reserved
        0x00, 0x00, 0x00, 0x00,
        // Byte 07h: Scan lines supported (bit 0=200, bit 1=350, bit 2=400)
        0x07, // All three scan line counts
        // Byte 08h: Total character blocks available
        0x02, // Byte 09h: Max active character blocks
        0x08,
        // Byte 0Ah: Misc flags #1
        // bit 0: all modes on all displays, bit 1: gray summing,
        // bit 2: char font loading, bit 3: default palette control,
        // bit 4: cursor emulation, bit 5: EGA palette, bit 6: color palette,
        // bit 7: color paging
        0xFF,
        // Byte 0Bh: Misc flags #2
        // bit 1: save/restore state, bit 2: blinking control, bit 3: DCC supported
        0x0E, // Bytes 0Ch-0Dh: Reserved
        0x00, 0x00, // Byte 0Eh: Save pointer flags
        0x00, // Byte 0Fh: Reserved
        0x00,
    ];

    for (i, &byte) in table.iter().enumerate() {
        bus.memory_write_u8(VGA_STATIC_FUNC_TABLE_ADDR + i, byte);
    }
}

fn bios_interrupt_handlers_reset(bus: &mut Bus) {
    for addr in (IVT_START..=IVT_END).step_by(IVT_ENTRY_SIZE) {
        let irq = addr / IVT_ENTRY_SIZE;
        bus.memory_write_u32(addr, ((BIOS_CODE_SEGMENT as u32) << 16) | irq as u32);
    }
}
