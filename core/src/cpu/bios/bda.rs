use crate::memory_bus::MemoryBus;

// BIOS Data Area (BDA) constants
pub const BDA_SEGMENT: u16 = 0x0040;
pub const BDA_START: usize = 0x0400; // Physical address (0x40 * 16)
pub const BDA_SIZE: usize = 0x100; // 256 bytes

// BDA field offsets (from 0x0040:0000)
pub const BDA_COM_PORTS: usize = 0x00; // COM1-COM4 port addresses (4 words)
pub const BDA_LPT_PORTS: usize = 0x08; // LPT1-LPT4 port addresses (4 words)
pub const BDA_EQUIPMENT_LIST: usize = 0x10; // Equipment list word
pub const BDA_MEMORY_SIZE: usize = 0x13; // Memory size in KB (word)
pub const BDA_KEYBOARD_FLAGS1: usize = 0x17; // Keyboard shift flags
pub const BDA_KEYBOARD_FLAGS2: usize = 0x18; // Keyboard shift flags
pub const BDA_KEYBOARD_BUFFER_HEAD: usize = 0x1A; // Keyboard buffer head pointer
pub const BDA_KEYBOARD_BUFFER_TAIL: usize = 0x1C; // Keyboard buffer tail pointer
pub const BDA_KEYBOARD_BUFFER: usize = 0x1E; // Keyboard buffer (32 bytes)
pub const BDA_VIDEO_MODE: usize = 0x49; // Current video mode
pub const BDA_SCREEN_COLUMNS: usize = 0x4A; // Number of screen columns
pub const BDA_VIDEO_PAGE_SIZE: usize = 0x4C; // Video page size in bytes
pub const BDA_VIDEO_PAGE_OFFSET: usize = 0x4E; // Current page start address
pub const BDA_CURSOR_POS: usize = 0x50; // Cursor positions for 8 pages (16 bytes)
pub const BDA_CURSOR_END_LINE: usize = 0x60; // Cursor end scan line
pub const BDA_CURSOR_START_LINE: usize = 0x61; // Cursor start scan line
pub const BDA_ACTIVE_PAGE: usize = 0x62; // Active display page
pub const BDA_CRTC_PORT: usize = 0x63; // CRT controller base port address
pub const BDA_CRT_MODE_CONTROL: usize = 0x65; // CRT mode control register
pub const BDA_CRT_PALETTE: usize = 0x66; // CRT palette register
pub const BDA_TIMER_COUNTER: usize = 0x6C; // Timer counter (dword) - ticks since midnight
pub const BDA_TIMER_OVERFLOW: usize = 0x70; // Timer midnight rollover flag (byte)
pub const BDA_NUM_HARD_DRIVES: usize = 0x75; // Number of hard drives installed (byte)
pub const BDA_KEYBOARD_BUFFER_START: usize = 0x80; // Keyboard buffer start pointer (word, normally 0x001E)
pub const BDA_KEYBOARD_BUFFER_END: usize = 0x82; // Keyboard buffer end pointer (word, normally 0x003E)
pub const BDA_EGA_ROWS: usize = 0x84; // EGA/VGA: number of rows on screen minus 1 (byte, e.g. 24 for 25-row mode)
pub const BDA_EGA_CHAR_HEIGHT: usize = 0x85; // EGA/VGA: bytes per character (byte, e.g. 16 for 8x16 font)
pub const BDA_MOUSE_X: usize = 0xE0; // Mouse X position (word)
pub const BDA_MOUSE_Y: usize = 0xE2; // Mouse Y position (word)
pub const BDA_MOUSE_BUTTONS: usize = 0xE4; // Mouse button state (byte)
pub const BDA_MOUSE_VISIBLE: usize = 0xE5; // Mouse cursor visibility counter (byte)
pub const BDA_MOUSE_MIN_X: usize = 0xE6; // Mouse horizontal minimum (word)
pub const BDA_MOUSE_MAX_X: usize = 0xE8; // Mouse horizontal maximum (word)
pub const BDA_MOUSE_MIN_Y: usize = 0xEA; // Mouse vertical minimum (word)
pub const BDA_MOUSE_MAX_Y: usize = 0xEC; // Mouse vertical maximum (word)

pub(in crate::cpu) fn bda_reset(memory_bus: &mut MemoryBus) {
    todo!();
}
