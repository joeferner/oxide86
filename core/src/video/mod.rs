pub mod font;
pub mod palette;
pub mod renderer;
pub mod text;
pub mod video_buffer;
pub mod video_card;

pub use video_buffer::VideoBuffer;
pub use video_card::VideoCard;

use crate::{
    bus::Bus,
    video::video_card::{VIDEO_CARD_REG_CURSOR_LOC_HIGH, VIDEO_CARD_REG_CURSOR_LOC_LOW},
};

pub const VIDEO_MODE_02H_COLOR_TEXT_80_X_25: u8 = 0x02;
pub const VIDEO_MODE_03H_COLOR_TEXT_80_X_25: u8 = 0x03;

// CGA video memory constants
pub const CGA_MEMORY_START: usize = 0xB8000;
pub const CGA_MEMORY_END: usize = 0xBFFFF;
pub const CGA_MEMORY_SIZE: usize = CGA_MEMORY_END - CGA_MEMORY_START + 1; // 32KB

// EGA planar memory: 16KB per plane × 4 planes = 64KB total.
// This supports two 320×200 display pages (8000 bytes each) plus tile storage at
// offsets 0x2000+, which games like Indiana Jones use for background tiles.
pub const EGA_PLANE_SIZE: usize = 0x4000; // 16KB per plane

// Video RAM size: 64KB total — shared between EGA planar (4 × 16KB = 64KB)
// and VGA mode 13h linear framebuffer (64000 bytes).
pub const VIDEO_MEMORY_SIZE: usize = EGA_PLANE_SIZE * 4; // 64KB

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
