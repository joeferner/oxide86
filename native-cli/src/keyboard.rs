use crossterm::event::KeyCode;
use oxide86_core::scan_code::SCAN_CODE_F12;

/// Map crossterm KeyCode to 8086 scan code
pub fn key_code_to_scan_code(code: &KeyCode) -> u8 {
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
        KeyCode::F(11) => 0x57,
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

/// Returns true if typing this character requires the Shift key on a US layout.
/// Some terminals don't report KeyModifiers::SHIFT for symbol characters, so
/// we infer it from the character itself.
pub fn char_requires_shift(c: char) -> bool {
    matches!(
        c,
        '~' | '!'
            | '@'
            | '#'
            | '$'
            | '%'
            | '^'
            | '&'
            | '*'
            | '('
            | ')'
            | '_'
            | '+'
            | '{'
            | '}'
            | '|'
            | ':'
            | '"'
            | '<'
            | '>'
            | '?'
            | 'A'..='Z'
    )
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
