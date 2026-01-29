use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use emu86_core::cpu::bios::KeyPress;
use log::debug;
use std::time::Duration;

// Console I/O operations for NativeBios

pub const SCAN_CODE_F12: u8 = 0x86;

/// Translate Unix line endings to DOS
fn translate_newline(ch: u8) -> u8 {
    if ch == 0x0A {
        0x0D // LF -> CR
    } else {
        ch
    }
}

pub fn read_char() -> Option<u8> {
    // Block until we get a key press
    loop {
        if let Ok(Event::Key(key_event)) = event::read()
            && let Some(ch) = key_event_to_ascii(&key_event)
        {
            return Some(translate_newline(ch));
        }
    }
}

/// Check if a character is available and return it (non-blocking)
/// Used by INT 21h, AH=06h (Direct Console I/O)
pub fn check_char() -> Option<u8> {
    if event::poll(Duration::from_millis(0)).ok()?
        && let Ok(Event::Key(key_event)) = event::read()
        && let Some(ch) = key_event_to_ascii(&key_event)
    {
        return Some(translate_newline(ch));
    }
    None
}

/// Check if a character is available without consuming it
/// Used by INT 21h, AH=0Bh (Check Input Status)
pub fn has_char_available() -> bool {
    event::poll(Duration::from_millis(0)).unwrap_or(false)
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

        // Regular characters - use ASCII value as scan code
        KeyCode::Char(c) if (0x20..0x7F).contains(&(*c as u8)) => *c as u8,

        _ => 0x00,
    }
}

pub fn read_key() -> Option<KeyPress> {
    // Block until we get a key press
    loop {
        if let Ok(Event::Key(key_event)) = event::read() {
            let key_press = key_event_to_keypress(&key_event);
            debug!("key press (read_key): 0x{:02X}", key_press.scan_code);
            return Some(key_press);
        }
    }
}

pub fn check_key() -> Option<KeyPress> {
    // Check if a key is available without blocking
    if event::poll(Duration::from_millis(0)).unwrap_or(false)
        && let Ok(Event::Key(key_event)) = event::read()
    {
        let key_press = key_event_to_keypress(&key_event);
        debug!("key press (check_key): 0x{:02X}", key_press.scan_code);
        return Some(key_press);
    }
    None
}

/// Convert crossterm KeyEvent to KeyPress with scan code and ASCII code
fn key_event_to_keypress(key_event: &KeyEvent) -> KeyPress {
    let scan_code = key_code_to_scan_code(&key_event.code);
    let ascii_code = key_event_to_ascii(key_event).unwrap_or(0x00);

    KeyPress {
        scan_code,
        ascii_code,
    }
}
