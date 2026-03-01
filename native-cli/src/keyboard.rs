use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxide86_core::KeyPress;

pub const SCAN_CODE_F12: u8 = 0x86;

/// Convert crossterm KeyEvent to KeyPress with scan code and ASCII code
pub fn key_event_to_keypress(key_event: &KeyEvent) -> KeyPress {
    let base_scan_code = key_code_to_scan_code(&key_event.code);
    let mut ascii_code = key_event_to_ascii(key_event).unwrap_or(0x00);

    // Apply modifier key adjustments for function keys
    let scan_code = apply_modifier_to_scan_code(
        base_scan_code,
        &key_event.code,
        &key_event.modifiers,
        &mut ascii_code,
    );

    KeyPress {
        scan_code,
        ascii_code,
    }
}

/// Convert crossterm KeyEvent to ASCII character (for simple key presses)
fn key_event_to_ascii(key_event: &KeyEvent) -> Option<u8> {
    match key_event.code {
        KeyCode::Char(c) => {
            // Handle Ctrl+letter combinations
            if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'a'..='z' => return Some(c as u8 - b'a' + 1), // Ctrl+A = 0x01, etc.
                    'A'..='Z' => return Some(c as u8 - b'A' + 1),
                    _ => {}
                }
            }
            // For Alt+letter combinations, return 0 (will be handled by apply_modifier_to_scan_code)
            if key_event.modifiers.contains(KeyModifiers::ALT) && c.is_ascii_alphabetic() {
                return Some(0x00);
            }
            // For Alt+number combinations, return 0 (will be handled by apply_modifier_to_scan_code)
            if key_event.modifiers.contains(KeyModifiers::ALT) && c.is_ascii_digit() {
                return Some(0x00);
            }
            Some(c as u8)
        }
        KeyCode::Enter => Some(0x0D),     // CR
        KeyCode::Backspace => Some(0x08), // BS
        KeyCode::Tab => Some(0x09),       // TAB
        KeyCode::Esc => Some(0x1B),       // ESC
        KeyCode::Delete => Some(0x7F),    // DEL
        _ => None,                        // Non-ASCII keys (arrows, function keys) return None
    }
}

/// Apply modifier keys (Shift, Ctrl, Alt) to scan codes
/// This is needed for function keys and Alt+letter combinations
fn apply_modifier_to_scan_code(
    base_scan_code: u8,
    key_code: &KeyCode,
    modifiers: &KeyModifiers,
    ascii_code: &mut u8,
) -> u8 {
    // Handle function keys with modifiers (F1-F10)
    if let KeyCode::F(n) = key_code {
        if *n >= 1 && *n <= 10 {
            // Base scan codes for F1-F10: 0x3B-0x44
            let base = 0x3B + *n - 1;

            return if modifiers.contains(KeyModifiers::ALT) {
                // Alt+F1..F10: 0x68-0x71
                *ascii_code = 0x00;
                0x68 + *n - 1
            } else if modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+F1..F10: 0x5E-0x67
                *ascii_code = 0x00;
                0x5E + *n - 1
            } else if modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+F1..F10: 0x54-0x5D
                *ascii_code = 0x00;
                0x54 + *n - 1
            } else {
                // No modifier
                *ascii_code = 0x00;
                base
            };
        } else if *n == 11 || *n == 12 {
            // F11 and F12 don't have standard shifted scan codes in BIOS
            // Just clear ASCII for these
            *ascii_code = 0x00;
            return base_scan_code;
        }
    }

    // Handle Alt+letter combinations
    if modifiers.contains(KeyModifiers::ALT)
        && let KeyCode::Char(c) = key_code
        && c.is_ascii_alphabetic()
    {
        // For Alt+letter, scan code is the letter's scan code, ASCII is 0
        *ascii_code = 0x00;
        return base_scan_code;
    }

    // Handle Alt+number combinations (top row 1-0)
    if modifiers.contains(KeyModifiers::ALT) {
        match key_code {
            KeyCode::Char('1') => {
                *ascii_code = 0x00;
                return 0x78;
            } // Alt+1
            KeyCode::Char('2') => {
                *ascii_code = 0x00;
                return 0x79;
            } // Alt+2
            KeyCode::Char('3') => {
                *ascii_code = 0x00;
                return 0x7A;
            } // Alt+3
            KeyCode::Char('4') => {
                *ascii_code = 0x00;
                return 0x7B;
            } // Alt+4
            KeyCode::Char('5') => {
                *ascii_code = 0x00;
                return 0x7C;
            } // Alt+5
            KeyCode::Char('6') => {
                *ascii_code = 0x00;
                return 0x7D;
            } // Alt+6
            KeyCode::Char('7') => {
                *ascii_code = 0x00;
                return 0x7E;
            } // Alt+7
            KeyCode::Char('8') => {
                *ascii_code = 0x00;
                return 0x7F;
            } // Alt+8
            KeyCode::Char('9') => {
                *ascii_code = 0x00;
                return 0x80;
            } // Alt+9
            KeyCode::Char('0') => {
                *ascii_code = 0x00;
                return 0x81;
            } // Alt+0
            _ => {}
        }
    }

    base_scan_code
}

/// Map crossterm KeyCode to 8086 scan code
fn key_code_to_scan_code(code: &KeyCode) -> u8 {
    match code {
        KeyCode::Esc => 0x01,
        KeyCode::Backspace => 0x0E,
        KeyCode::Tab => 0x0F,
        KeyCode::Enter => 0x1C,
        KeyCode::Delete => 0x53,

        // Function keys
        KeyCode::F(1) => 0x3B,
        KeyCode::F(2) => 0x3C,
        KeyCode::F(3) => 0x3D,
        KeyCode::F(4) => 0x3E,
        KeyCode::F(5) => 0x3F,
        KeyCode::F(6) => 0x40,
        KeyCode::F(7) => 0x41,
        KeyCode::F(8) => 0x42,
        KeyCode::F(9) => 0x43,
        KeyCode::F(10) => 0x44,
        KeyCode::F(11) => 0x85,
        KeyCode::F(12) => SCAN_CODE_F12,

        // Arrow keys
        KeyCode::Up => 0x48,
        KeyCode::Down => 0x50,
        KeyCode::Left => 0x4B,
        KeyCode::Right => 0x4D,

        // Navigation keys
        KeyCode::Home => 0x47,
        KeyCode::End => 0x4F,
        KeyCode::PageUp => 0x49,
        KeyCode::PageDown => 0x51,
        KeyCode::Insert => 0x52,

        // Character keys - map to physical keyboard scan codes
        KeyCode::Char(c) => char_to_scan_code(*c),

        _ => 0x00,
    }
}

/// Map a character to its physical keyboard scan code
/// This is based on the physical position of keys on an IBM PC keyboard
fn char_to_scan_code(c: char) -> u8 {
    match c.to_ascii_uppercase() {
        // Top row (number keys)
        '1' | '!' => 0x02,
        '2' | '@' => 0x03,
        '3' | '#' => 0x04,
        '4' | '$' => 0x05,
        '5' | '%' => 0x06,
        '6' | '^' => 0x07,
        '7' | '&' => 0x08,
        '8' | '*' => 0x09,
        '9' | '(' => 0x0A,
        '0' | ')' => 0x0B,
        '-' | '_' => 0x0C,
        '=' | '+' => 0x0D,

        // QWERTY row
        'Q' => 0x10,
        'W' => 0x11,
        'E' => 0x12,
        'R' => 0x13,
        'T' => 0x14,
        'Y' => 0x15,
        'U' => 0x16,
        'I' => 0x17,
        'O' => 0x18,
        'P' => 0x19,
        '[' | '{' => 0x1A,
        ']' | '}' => 0x1B,

        // ASDF row
        'A' => 0x1E,
        'S' => 0x1F,
        'D' => 0x20,
        'F' => 0x21,
        'G' => 0x22,
        'H' => 0x23,
        'J' => 0x24,
        'K' => 0x25,
        'L' => 0x26,
        ';' | ':' => 0x27,
        '\'' | '"' => 0x28,
        '`' | '~' => 0x29,

        // ZXCV row
        '\\' | '|' => 0x2B,
        'Z' => 0x2C,
        'X' => 0x2D,
        'C' => 0x2E,
        'V' => 0x2F,
        'B' => 0x30,
        'N' => 0x31,
        'M' => 0x32,
        ',' | '<' => 0x33,
        '.' | '>' => 0x34,
        '/' | '?' => 0x35,

        // Space bar
        ' ' => 0x39,

        _ => 0x00,
    }
}
