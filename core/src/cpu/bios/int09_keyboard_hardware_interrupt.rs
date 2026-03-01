use crate::{
    KeyPress,
    bus::Bus,
    cpu::{Cpu, bios::bda::bda_add_key_to_buffer},
    devices::keyboard_controller::KEYBOARD_IO_PORT_DATA,
};

impl Cpu {
    /// INT 09h - Keyboard Hardware Interrupt
    ///
    /// This is the default BIOS handler that reads keyboard data and adds it to the buffer.
    /// Programs with custom INT 09h handlers will replace this via the IVT and handle
    /// keyboard input directly by reading port 0x60.
    pub(in crate::cpu) fn handle_int09_keyboard_hardware_interrupt(&mut self, bus: &mut Bus) {
        let scan_code = bus.io_read_u8(KEYBOARD_IO_PORT_DATA);

        // Check if this is a key release (bit 7 set)
        // Key releases should NOT be added to the BIOS buffer - they're only for custom handlers
        if scan_code & 0x80 != 0 {
            log::debug!(
                "INT 09h (BIOS): Key release detected (scan=0x{:02X}), not buffering",
                scan_code
            );
            return;
        }

        let ascii_code = scan_code_to_ascii(scan_code);
        bda_add_key_to_buffer(
            bus,
            KeyPress {
                scan_code,
                ascii_code,
            },
        );
    }
}

/// Convert an XT keyboard scan code (set 1, make code) to an ASCII character.
///
/// Returns 0x00 for keys with no direct ASCII representation (modifiers, function keys,
/// navigation keys). The caller should treat 0x00 as an extended keycode where INT 16h
/// will return AH=scan_code, AL=0x00.
pub fn scan_code_to_ascii(scan_code: u8) -> u8 {
    match scan_code {
        0x01 => 0x1B, // Escape
        0x02 => b'1',
        0x03 => b'2',
        0x04 => b'3',
        0x05 => b'4',
        0x06 => b'5',
        0x07 => b'6',
        0x08 => b'7',
        0x09 => b'8',
        0x0A => b'9',
        0x0B => b'0',
        0x0C => b'-',
        0x0D => b'=',
        0x0E => 0x08, // Backspace
        0x0F => 0x09, // Tab
        0x10 => b'q',
        0x11 => b'w',
        0x12 => b'e',
        0x13 => b'r',
        0x14 => b't',
        0x15 => b'y',
        0x16 => b'u',
        0x17 => b'i',
        0x18 => b'o',
        0x19 => b'p',
        0x1A => b'[',
        0x1B => b']',
        0x1C => 0x0D, // Enter
        0x1D => 0x00, // Left Ctrl
        0x1E => b'a',
        0x1F => b's',
        0x20 => b'd',
        0x21 => b'f',
        0x22 => b'g',
        0x23 => b'h',
        0x24 => b'j',
        0x25 => b'k',
        0x26 => b'l',
        0x27 => b';',
        0x28 => b'\'',
        0x29 => b'`',
        0x2A => 0x00, // Left Shift
        0x2B => b'\\',
        0x2C => b'z',
        0x2D => b'x',
        0x2E => b'c',
        0x2F => b'v',
        0x30 => b'b',
        0x31 => b'n',
        0x32 => b'm',
        0x33 => b',',
        0x34 => b'.',
        0x35 => b'/',
        0x36 => 0x00,        // Right Shift
        0x37 => b'*',        // Numpad * / PrtScr
        0x38 => 0x00,        // Left Alt
        0x39 => b' ',        // Space
        0x3A => 0x00,        // Caps Lock
        0x3B..=0x44 => 0x00, // F1–F10
        0x45 => 0x00,        // Num Lock
        0x46 => 0x00,        // Scroll Lock
        0x47 => 0x00,        // Numpad 7 / Home
        0x48 => 0x00,        // Numpad 8 / Up
        0x49 => 0x00,        // Numpad 9 / PgUp
        0x4A => b'-',        // Numpad -
        0x4B => 0x00,        // Numpad 4 / Left
        0x4C => 0x00,        // Numpad 5
        0x4D => 0x00,        // Numpad 6 / Right
        0x4E => b'+',        // Numpad +
        0x4F => 0x00,        // Numpad 1 / End
        0x50 => 0x00,        // Numpad 2 / Down
        0x51 => 0x00,        // Numpad 3 / PgDn
        0x52 => 0x00,        // Numpad 0 / Ins
        0x53 => 0x7F,        // Numpad . / Del
        _ => 0x00,
    }
}
