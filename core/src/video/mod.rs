pub mod font;
pub mod mode;
pub mod palette;
pub mod renderer;
pub mod text;
pub mod video_buffer;
pub mod video_card;
pub mod video_card_type;

pub use mode::Mode;
pub use video_buffer::VideoBuffer;
pub use video_card::VideoCard;
pub use video_card_type::VideoCardType;

use crate::{
    bus::Bus,
    video::video_card::{VIDEO_CARD_REG_CURSOR_LOC_HIGH, VIDEO_CARD_REG_CURSOR_LOC_LOW},
};

// EGA video memory constants (planar, at A000:0000)
pub const EGA_MEMORY_START: usize = 0xA0000;
pub const EGA_MEMORY_END: usize = 0xAFFFF;

// MDA/HGC video memory constants (monochrome adapters at B000:0000)
pub const MDA_MEMORY_START: usize = 0xB0000;
pub const MDA_MEMORY_END: usize = 0xB7FFF;
pub const MDA_MEMORY_SIZE: usize = MDA_MEMORY_END - MDA_MEMORY_START + 1; // 32KB

// CGA video memory constants
pub const CGA_MEMORY_START: usize = 0xB8000;
pub const CGA_MEMORY_END: usize = 0xBFFFF;
pub const CGA_MEMORY_SIZE: usize = CGA_MEMORY_END - CGA_MEMORY_START + 1; // 32KB

// EGA planar memory: 64KB per plane × 4 planes = 256KB total (real hardware).
// Mode 0Dh (320×200) uses 8000 bytes/plane for the visible area.
// Games like Commander Keen use the full 64KB window: tile data lives at offsets
// above 0x8000 (e.g. A700:0000 = offset 0x7000, extending to 0xE4DF per plane).
pub const EGA_PLANE_SIZE: usize = 0x10000; // 64KB per plane

// Video RAM size: 256KB total — 4 EGA planes × 64KB, also covers
// VGA mode 13h linear framebuffer (64000 bytes).
pub const VIDEO_MEMORY_SIZE: usize = EGA_PLANE_SIZE * 4; // 256KB

// VGA mode 13h: 320×200 = 64000 bytes linear framebuffer at A000:0000
pub const VGA_MODE_13_WIDTH: usize = 320;
pub const VGA_MODE_13_HEIGHT: usize = 200;
pub const VGA_MODE_13_FRAMEBUFFER_SIZE: usize = VGA_MODE_13_WIDTH * VGA_MODE_13_HEIGHT;

pub const TEXT_MODE_COLS: usize = 80;
pub const TEXT_MODE_ROWS: usize = 25;
pub const TEXT_MODE_BYTES_PER_CHAR: usize = 2;
pub const TEXT_MODE_SIZE: usize = TEXT_MODE_COLS * TEXT_MODE_ROWS * TEXT_MODE_BYTES_PER_CHAR;

pub const DEFAULT_CURSOR_START_LINE: u8 = 0x0c;
pub const DEFAULT_CURSOR_END_LINE: u8 = 0x0d;

// VGA color constants
pub mod colors {
    pub const BLACK: u8 = 0x0;
    pub const BLUE: u8 = 0x1;
    pub const GREEN: u8 = 0x2;
    pub const CYAN: u8 = 0x3;
    pub const RED: u8 = 0x4;
    pub const MAGENTA: u8 = 0x5;
    pub const BROWN: u8 = 0x6;
    pub const LIGHT_GRAY: u8 = 0x7;
    pub const DARK_GRAY: u8 = 0x8;
    pub const LIGHT_BLUE: u8 = 0x9;
    pub const LIGHT_GREEN: u8 = 0xA;
    pub const LIGHT_CYAN: u8 = 0xB;
    pub const LIGHT_RED: u8 = 0xC;
    pub const LIGHT_MAGENTA: u8 = 0xD;
    pub const YELLOW: u8 = 0xE;
    pub const WHITE: u8 = 0xF;
}

pub(crate) fn video_set_cursor_pos(bus: &mut Bus, crt_controller_port: u16, linear_offset: u16) {
    // Send the HIGH byte (Registers 0x0E)
    // Tell the VGA controller we want to update the "Cursor Location High" register
    bus.io_write_u8(crt_controller_port, VIDEO_CARD_REG_CURSOR_LOC_HIGH);
    // Send the actual high 8 bits of our offset
    bus.io_write_u8(crt_controller_port + 1, ((linear_offset >> 8) & 0xFF) as u8);

    // Send the LOW byte (Register 0x0F)
    // Tell the VGA controller we want to update the "Cursor Location Low" register
    bus.io_write_u8(crt_controller_port, VIDEO_CARD_REG_CURSOR_LOC_LOW);
    // Send the actual low 8 bits of our offset
    bus.io_write_u8(crt_controller_port + 1, (linear_offset & 0xFF) as u8);
}

pub(crate) fn video_calculate_linear_offset(row: u8, col: u8, max_cols: u8) -> u16 {
    (row as u16 * max_cols as u16) + col as u16
}
