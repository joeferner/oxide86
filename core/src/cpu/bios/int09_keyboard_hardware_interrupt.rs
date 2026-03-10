use crate::{
    KeyPress,
    bus::Bus,
    cpu::{
        Cpu,
        bios::bda::{
            BDA_KEYBOARD_FLAGS1_ALT, BDA_KEYBOARD_FLAGS1_CTRL, BDA_KEYBOARD_FLAGS1_LEFT_SHIFT,
            BDA_KEYBOARD_FLAGS1_RIGHT_SHIFT, bda_add_key_to_buffer, bda_get_keyboard_flags1,
            bda_set_keyboard_flags1,
        },
    },
    devices::{
        keyboard_controller::KEYBOARD_IO_PORT_DATA,
        pic::{PIC_COMMAND_EOI, PIC_IO_PORT_COMMAND},
    },
    scan_code::{
        SCAN_CODE_LEFT_ALT, SCAN_CODE_LEFT_ALT_RELEASE, SCAN_CODE_LEFT_CTRL,
        SCAN_CODE_LEFT_CTRL_RELEASE, SCAN_CODE_LEFT_SHIFT, SCAN_CODE_LEFT_SHIFT_RELEASE,
        SCAN_CODE_RIGHT_SHIFT, SCAN_CODE_RIGHT_SHIFT_RELEASE,
    },
};

impl Cpu {
    /// INT 09h - Keyboard Hardware Interrupt
    ///
    /// This is the default BIOS handler that reads keyboard data and adds it to the buffer.
    /// Programs with custom INT 09h handlers will replace this via the IVT and handle
    /// keyboard input directly by reading port 0x60.
    pub(in crate::cpu) fn handle_int09_keyboard_hardware_interrupt(&mut self, bus: &mut Bus) {
        bus.increment_cycle_count(300);
        // Acknowledge interrupt to PIC so it can fire again
        bus.io_write_u8(PIC_IO_PORT_COMMAND, PIC_COMMAND_EOI);

        let scan_code = bus.io_read_u8(KEYBOARD_IO_PORT_DATA);

        // Handle modifier key press/release - update BDA flags but don't buffer
        match scan_code {
            SCAN_CODE_LEFT_SHIFT => {
                let flags = bda_get_keyboard_flags1(bus) | BDA_KEYBOARD_FLAGS1_LEFT_SHIFT;
                bda_set_keyboard_flags1(bus, flags);
                log::debug!("INT 0x09 (BIOS): Left Shift pressed, flags=0x{:02X}", flags);
                return;
            }
            SCAN_CODE_RIGHT_SHIFT => {
                let flags = bda_get_keyboard_flags1(bus) | BDA_KEYBOARD_FLAGS1_RIGHT_SHIFT;
                bda_set_keyboard_flags1(bus, flags);
                log::debug!(
                    "INT 0x09 (BIOS): Right Shift pressed, flags=0x{:02X}",
                    flags
                );
                return;
            }
            SCAN_CODE_LEFT_SHIFT_RELEASE => {
                let flags = bda_get_keyboard_flags1(bus) & !BDA_KEYBOARD_FLAGS1_LEFT_SHIFT;
                bda_set_keyboard_flags1(bus, flags);
                log::debug!(
                    "INT 0x09 (BIOS): Left Shift released, flags=0x{:02X}",
                    flags
                );
                return;
            }
            SCAN_CODE_RIGHT_SHIFT_RELEASE => {
                let flags = bda_get_keyboard_flags1(bus) & !BDA_KEYBOARD_FLAGS1_RIGHT_SHIFT;
                bda_set_keyboard_flags1(bus, flags);
                log::debug!(
                    "INT 0x09 (BIOS): Right Shift released, flags=0x{:02X}",
                    flags
                );
                return;
            }
            SCAN_CODE_LEFT_CTRL => {
                let flags = bda_get_keyboard_flags1(bus) | BDA_KEYBOARD_FLAGS1_CTRL;
                bda_set_keyboard_flags1(bus, flags);
                log::debug!("INT 0x09 (BIOS): Left Ctrl pressed, flags=0x{:02X}", flags);
                return;
            }
            SCAN_CODE_LEFT_CTRL_RELEASE => {
                let flags = bda_get_keyboard_flags1(bus) & !BDA_KEYBOARD_FLAGS1_CTRL;
                bda_set_keyboard_flags1(bus, flags);
                log::debug!("INT 0x09 (BIOS): Left Ctrl released, flags=0x{:02X}", flags);
                return;
            }
            SCAN_CODE_LEFT_ALT => {
                let flags = bda_get_keyboard_flags1(bus) | BDA_KEYBOARD_FLAGS1_ALT;
                bda_set_keyboard_flags1(bus, flags);
                log::debug!("INT 0x09 (BIOS): Left Alt pressed, flags=0x{:02X}", flags);
                return;
            }
            SCAN_CODE_LEFT_ALT_RELEASE => {
                let flags = bda_get_keyboard_flags1(bus) & !BDA_KEYBOARD_FLAGS1_ALT;
                bda_set_keyboard_flags1(bus, flags);
                log::debug!("INT 0x09 (BIOS): Left Alt released, flags=0x{:02X}", flags);
                return;
            }
            _ => {}
        }

        // Check if this is any other key release (bit 7 set)
        // Key releases should NOT be added to the BIOS buffer - they're only for custom handlers
        if scan_code & 0x80 != 0 {
            log::debug!(
                "INT 0x09 (BIOS): Key release detected (scan=0x{:02X}), not buffering",
                scan_code
            );
            return;
        }

        let flags = bda_get_keyboard_flags1(bus);
        let shifted =
            flags & (BDA_KEYBOARD_FLAGS1_LEFT_SHIFT | BDA_KEYBOARD_FLAGS1_RIGHT_SHIFT) != 0;
        let ctrl = flags & BDA_KEYBOARD_FLAGS1_CTRL != 0;
        let alt = flags & BDA_KEYBOARD_FLAGS1_ALT != 0;
        let ascii_code = scan_code_to_ascii(scan_code, shifted, ctrl, alt);
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
pub(crate) fn scan_code_to_ascii(scan_code: u8, shifted: bool, ctrl: bool, alt: bool) -> u8 {
    // Alt combinations: ascii=0x00, caller uses scan_code as extended key identifier
    if alt {
        return 0x00;
    }

    // Ctrl combinations: generate ASCII control codes (0x00-0x1F)
    if ctrl {
        return match scan_code {
            0x03 => 0x00, // Ctrl+2 / Ctrl+@ = NUL
            0x07 => 0x1E, // Ctrl+6 (^) = RS
            0x0C => 0x1F, // Ctrl+- = US
            0x10 => 0x11, // Ctrl+Q = DC1
            0x11 => 0x17, // Ctrl+W = ETB
            0x12 => 0x05, // Ctrl+E = ENQ
            0x13 => 0x12, // Ctrl+R = DC2
            0x14 => 0x14, // Ctrl+T = DC4
            0x15 => 0x19, // Ctrl+Y = EM
            0x16 => 0x15, // Ctrl+U = NAK
            0x17 => 0x09, // Ctrl+I = HT (tab)
            0x18 => 0x0F, // Ctrl+O = SI
            0x19 => 0x10, // Ctrl+P = DLE
            0x1A => 0x1B, // Ctrl+[ = ESC
            0x1B => 0x1D, // Ctrl+] = GS
            0x1E => 0x01, // Ctrl+A = SOH
            0x1F => 0x13, // Ctrl+S = DC3
            0x20 => 0x04, // Ctrl+D = EOT
            0x21 => 0x06, // Ctrl+F = ACK
            0x22 => 0x07, // Ctrl+G = BEL
            0x23 => 0x08, // Ctrl+H = BS
            0x24 => 0x0A, // Ctrl+J = LF
            0x25 => 0x0B, // Ctrl+K = VT
            0x26 => 0x0C, // Ctrl+L = FF
            0x2B => 0x1C, // Ctrl+\ = FS
            0x2C => 0x1A, // Ctrl+Z = SUB
            0x2D => 0x18, // Ctrl+X = CAN
            0x2E => 0x03, // Ctrl+C = ETX
            0x2F => 0x16, // Ctrl+V = SYN
            0x30 => 0x02, // Ctrl+B = STX
            0x31 => 0x0E, // Ctrl+N = SO
            0x32 => 0x0D, // Ctrl+M = CR
            0x39 => 0x00, // Ctrl+Space = NUL
            _ => 0x00,
        };
    }

    if shifted {
        match scan_code {
            0x01 => 0x1B, // Escape (unchanged)
            0x02 => b'!',
            0x03 => b'@',
            0x04 => b'#',
            0x05 => b'$',
            0x06 => b'%',
            0x07 => b'^',
            0x08 => b'&',
            0x09 => b'*',
            0x0A => b'(',
            0x0B => b')',
            0x0C => b'_',
            0x0D => b'+',
            0x0E => 0x08, // Backspace (unchanged)
            0x0F => 0x09, // Tab (unchanged)
            0x10 => b'Q',
            0x11 => b'W',
            0x12 => b'E',
            0x13 => b'R',
            0x14 => b'T',
            0x15 => b'Y',
            0x16 => b'U',
            0x17 => b'I',
            0x18 => b'O',
            0x19 => b'P',
            0x1A => b'{',
            0x1B => b'}',
            0x1C => 0x0D, // Enter (unchanged)
            0x1D => 0x00, // Left Ctrl
            0x1E => b'A',
            0x1F => b'S',
            0x20 => b'D',
            0x21 => b'F',
            0x22 => b'G',
            0x23 => b'H',
            0x24 => b'J',
            0x25 => b'K',
            0x26 => b'L',
            0x27 => b':',
            0x28 => b'"',
            0x29 => b'~',
            0x2A => 0x00, // Left Shift
            0x2B => b'|',
            0x2C => b'Z',
            0x2D => b'X',
            0x2E => b'C',
            0x2F => b'V',
            0x30 => b'B',
            0x31 => b'N',
            0x32 => b'M',
            0x33 => b'<',
            0x34 => b'>',
            0x35 => b'?',
            0x36 => 0x00,        // Right Shift
            0x37 => b'*',        // Numpad * / PrtScr (unchanged)
            0x38 => 0x00,        // Left Alt
            0x39 => b' ',        // Space (unchanged)
            0x3A => 0x00,        // Caps Lock
            0x3B..=0x44 => 0x00, // F1–F10
            0x45 => 0x00,        // Num Lock
            0x46 => 0x00,        // Scroll Lock
            0x47 => 0x00,        // Numpad 7 / Home
            0x48 => 0x00,        // Numpad 8 / Up
            0x49 => 0x00,        // Numpad 9 / PgUp
            0x4A => b'-',        // Numpad - (unchanged)
            0x4B => 0x00,        // Numpad 4 / Left
            0x4C => 0x00,        // Numpad 5
            0x4D => 0x00,        // Numpad 6 / Right
            0x4E => b'+',        // Numpad + (unchanged)
            0x4F => 0x00,        // Numpad 1 / End
            0x50 => 0x00,        // Numpad 2 / Down
            0x51 => 0x00,        // Numpad 3 / PgDn
            0x52 => 0x00,        // Numpad 0 / Ins
            0x53 => 0x7F,        // Numpad . / Del (unchanged)
            _ => 0x00,
        }
    } else {
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
}
