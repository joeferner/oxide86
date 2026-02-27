use crate::{
    memory_bus::MemoryBus,
    video::{
        TEXT_MODE_BYTES_PER_CHAR, TEXT_MODE_COLS, TEXT_MODE_ROWS, VIDEO_MODE_03H_COLOR_TEXT_80_X_25,
    },
};

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

// Equipment list bits
pub const EQUIPMENT_FLOPPY_INSTALLED: u16 = 0x0001;
pub const EQUIPMENT_MATH_COPROCESSOR: u16 = 0x0002;
pub const EQUIPMENT_POINTING_DEVICE: u16 = 0x0004; // PS/2 mouse
pub const EQUIPMENT_VIDEO_MODE_MASK: u16 = 0x0030; // Bits 4-5: initial video mode
pub const EQUIPMENT_VIDEO_MODE_80X25_COLOR: u16 = 0x0020;
pub const EQUIPMENT_VIDEO_MODE_80X25_MONO: u16 = 0x0030;
pub const EQUIPMENT_FLOPPY_COUNT_MASK: u16 = 0x00C0; // Bits 6-7: number of floppies - 1
pub const EQUIPMENT_SERIAL_COUNT_MASK: u16 = 0x0E00; // Bits 9-11: number of serial ports
pub const EQUIPMENT_PRINTER_COUNT_MASK: u16 = 0xC000; // Bits 14-15: number of printers

pub(in crate::cpu) fn bda_reset(memory_bus: &mut MemoryBus) {
    // COM port addresses (0x0040:0000 - 4 words)
    // Standard COM port I/O addresses
    memory_bus.write_u16(BDA_START + BDA_COM_PORTS, 0x03F8); // COM1
    memory_bus.write_u16(BDA_START + BDA_COM_PORTS + 2, 0x02F8); // COM2
    memory_bus.write_u16(BDA_START + BDA_COM_PORTS + 4, 0x03E8); // COM3
    memory_bus.write_u16(BDA_START + BDA_COM_PORTS + 6, 0x02E8); // COM4

    // LPT port addresses (0x0040:0008 - 4 words)
    // Standard LPT (parallel) port I/O addresses
    memory_bus.write_u16(BDA_START + BDA_LPT_PORTS, 0x0378); // LPT1
    memory_bus.write_u16(BDA_START + BDA_LPT_PORTS + 2, 0x0278); // LPT2
    memory_bus.write_u16(BDA_START + BDA_LPT_PORTS + 4, 0x03BC); // LPT3
    memory_bus.write_u16(BDA_START + BDA_LPT_PORTS + 6, 0x0000); // LPT4 (not installed)

    // Equipment list word (0x0040:0010)
    // Bits indicate installed hardware
    let mut equipment = 0u16;
    equipment |= EQUIPMENT_FLOPPY_INSTALLED; // Floppy drive installed
    equipment |= EQUIPMENT_VIDEO_MODE_80X25_COLOR; // 80x25 color text mode
    equipment |= 0x0040; // 1 floppy drive (bits 6-7: count-1 = 0)
    // No math coprocessor, no serial ports configured in equipment list
    memory_bus.write_u16(BDA_START + BDA_EQUIPMENT_LIST, equipment);

    // Memory size in KB (0x0040:0013)
    // Report 640KB of conventional memory (maximum for PC/XT compatibility)
    memory_bus.write_u16(BDA_START + BDA_MEMORY_SIZE, 640); // TODO verify 640 is the max number here

    // Keyboard flags (0x0040:0017-0018)
    memory_bus.write_u8(BDA_START + BDA_KEYBOARD_FLAGS1, 0); // No shift/ctrl/alt pressed
    memory_bus.write_u8(BDA_START + BDA_KEYBOARD_FLAGS2, 0); // No special states

    // Keyboard buffer pointers (0x0040:001A-001D)
    // Buffer is empty, both pointers point to buffer start
    let buffer_start = 0x1E; // Offset within BDA
    memory_bus.write_u16(BDA_START + BDA_KEYBOARD_BUFFER_HEAD, buffer_start);
    memory_bus.write_u16(BDA_START + BDA_KEYBOARD_BUFFER_TAIL, buffer_start);

    // Keyboard buffer (0x0040:001E-003D) - 32 bytes (16 scan code/char pairs)
    // Initialize to zeros
    for i in 0..32 {
        memory_bus.write_u8(BDA_START + BDA_KEYBOARD_BUFFER + i, 0);
    }

    // Video mode (0x0040:0049)
    memory_bus.write_u8(
        BDA_START + BDA_VIDEO_MODE,
        VIDEO_MODE_03H_COLOR_TEXT_80_X_25,
    );

    // Screen columns (0x0040:004A)
    memory_bus.write_u16(BDA_START + BDA_SCREEN_COLUMNS, TEXT_MODE_COLS as u16);

    // Video page size (0x0040:004C)
    memory_bus.write_u16(
        BDA_START + BDA_VIDEO_PAGE_SIZE,
        (TEXT_MODE_COLS * TEXT_MODE_ROWS * TEXT_MODE_BYTES_PER_CHAR) as u16,
    ); // 80*25*2 bytes

    // Current video page offset (0x0040:004E)
    memory_bus.write_u16(BDA_START + BDA_VIDEO_PAGE_OFFSET, 0); // Page 0

    // Cursor positions for 8 pages (0x0040:0050-005F)
    // Each page gets a word: low byte = column, high byte = row
    for page in 0..8 {
        memory_bus.write_u16(BDA_START + BDA_CURSOR_POS + page * 2, 0x0000); // Row 0, Col 0
    }

    // Cursor shape (0x0040:0060-0061)
    memory_bus.write_u8(BDA_START + BDA_CURSOR_END_LINE, 0x0D); // Cursor end scan line
    memory_bus.write_u8(BDA_START + BDA_CURSOR_START_LINE, 0x0C); // Cursor start scan line

    // Active display page (0x0040:0062)
    memory_bus.write_u8(BDA_START + BDA_ACTIVE_PAGE, 0); // Page 0

    // CRT controller port address (0x0040:0063)
    memory_bus.write_u16(BDA_START + BDA_CRTC_PORT, 0x03D4); // Color adapter (monochrome = 0x03B4)

    // CRT mode control register (0x0040:0065)
    memory_bus.write_u8(BDA_START + BDA_CRT_MODE_CONTROL, 0x09); // 80x25 text, enable video

    // CRT palette register (0x0040:0066)
    memory_bus.write_u8(BDA_START + BDA_CRT_PALETTE, 0x00); // Default palette

    // Timer counter (0x0040:006C) - 4 bytes
    // Initialize to 0 ticks since midnight
    memory_bus.write_u16(BDA_START + BDA_TIMER_COUNTER, 0); // Low word
    memory_bus.write_u16(BDA_START + BDA_TIMER_COUNTER + 2, 0); // High word

    // Timer overflow flag (0x0040:0070)
    memory_bus.write_u8(BDA_START + BDA_TIMER_OVERFLOW, 0); // No midnight rollover yet

    // Keyboard buffer range (0x0040:0080-0083)
    // These are the start and end pointers of the circular keyboard buffer in BDA
    memory_bus.write_u16(BDA_START + BDA_KEYBOARD_BUFFER_START, 0x001E); // Buffer starts at BDA+0x1E
    memory_bus.write_u16(BDA_START + BDA_KEYBOARD_BUFFER_END, 0x003E); // Buffer ends at BDA+0x3E

    // EGA/VGA rows and character height (0x0040:0084-0085)
    // Programs (e.g., Turbo Pascal, dBASE) read these to determine screen dimensions
    memory_bus.write_u8(BDA_START + BDA_EGA_ROWS, 24); // 25 rows - 1 = 24
    memory_bus.write_u8(BDA_START + BDA_EGA_CHAR_HEIGHT, 16); // 8x16 VGA font

    // Mouse position (0x0040:00E0-00E3) - custom emulator area, not standard BDA
    // Initialize to center of default 640x200 resolution
    memory_bus.write_u16(BDA_START + BDA_MOUSE_X, 320); // Center X
    memory_bus.write_u16(BDA_START + BDA_MOUSE_Y, 100); // Center Y

    // Mouse button state (0x0040:00E4)
    memory_bus.write_u8(BDA_START + BDA_MOUSE_BUTTONS, 0); // No buttons pressed

    // Mouse cursor visibility counter (0x0040:00E5)
    // Counter < 0 means hidden, >= 0 means visible
    // Initialize to -1 (hidden by default)
    memory_bus.write_u8(BDA_START + BDA_MOUSE_VISIBLE, 0xFF); // -1 as unsigned byte

    // Mouse coordinate boundaries (0x0040:00E6-00ED)
    // Default to 640x200 DOS graphics resolution
    memory_bus.write_u16(BDA_START + BDA_MOUSE_MIN_X, 0); // Minimum X
    memory_bus.write_u16(BDA_START + BDA_MOUSE_MAX_X, 639); // Maximum X
    memory_bus.write_u16(BDA_START + BDA_MOUSE_MIN_Y, 0); // Minimum Y
    memory_bus.write_u16(BDA_START + BDA_MOUSE_MAX_Y, 199); // Maximum Y
}

pub(in crate::cpu) fn bda_get_cursor_pos(memory_bus: &MemoryBus) -> (u8, u8) {
    // low byte = column, high byte = row
    let pos = memory_bus.read_u16(BDA_START + BDA_CURSOR_POS);
    let col = (pos & 0xff) as u8;
    let row = ((pos >> 8) & 0xff) as u8;
    return (row, col);
}

pub(in crate::cpu) fn bda_set_cursor_pos(memory_bus: &mut MemoryBus, row: u8, col: u8) {
    // low byte = column, high byte = row
    let pos = ((row as u16) << 8) | col as u16;
    memory_bus.write_u16(BDA_START + BDA_CURSOR_POS, pos);
}

pub(in crate::cpu) fn bda_get_columns(memory_bus: &MemoryBus) -> u16 {
    memory_bus.read_u16(BDA_START + BDA_SCREEN_COLUMNS)
}

pub(in crate::cpu) fn bda_get_rows(memory_bus: &MemoryBus) -> u8 {
    let rows = memory_bus.read_u8(BDA_START + BDA_EGA_ROWS);
    // On very old original IBM PCs (1981), the byte at 0x84 wasn't always
    // initialized because 25 lines was the only option. However, for any
    // EGA, VGA, or modern BIOS/UEFI CSM, this byte is the "source of truth."
    if rows == 0 {
        24 // 25 rows - 1 = 24
    } else {
        rows
    }
}

pub(in crate::cpu) fn bda_get_crt_controller_port_address(memory_bus: &MemoryBus) -> u16 {
    memory_bus.read_u16(BDA_START + BDA_CRTC_PORT)
}
