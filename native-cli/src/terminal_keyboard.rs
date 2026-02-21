//! Terminal-based keyboard input implementation for native CLI.
//!
//! This module provides a terminal-based keyboard input implementation that:
//! - Implements the KeyboardInput trait for platform-independent keyboard handling
//! - Handles F12 key interception for command mode (CLI-specific feature)
//! - Buffers keyboard input during polling to prevent key loss
//! - Uses crossterm for cross-platform terminal keyboard access

use oxide86_core::cpu::bios::KeyPress;
use oxide86_core::keyboard::KeyboardInput;
use std::collections::VecDeque;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub const SCAN_CODE_F12: u8 = 0x86;

/// Terminal-based keyboard input for native CLI.
///
/// This struct manages keyboard input from the terminal, with special handling
/// for F12 key presses which trigger command mode in the CLI interface.
pub struct TerminalKeyboard {
    /// Flag set when F12 is pressed (command mode request)
    command_mode_requested: bool,
    /// Buffer for keyboard input read during polling (excluding F12)
    keyboard_buffer: VecDeque<KeyPress>,
}

impl TerminalKeyboard {
    /// Create a new TerminalKeyboard instance.
    pub fn new() -> Self {
        Self {
            command_mode_requested: false,
            keyboard_buffer: VecDeque::new(),
        }
    }

    /// Process a single crossterm event.
    ///
    /// This method is used by centralized event polling to dispatch keyboard
    /// events. It handles F12 detection for command mode and buffers other keys.
    ///
    /// # Parameters
    ///
    /// - `event`: A crossterm Event to process
    pub fn process_crossterm_event(&mut self, event: Event) {
        if let Event::Key(key_event) = event {
            if key_event.kind != KeyEventKind::Press {
                return;
            }
            let key = key_event_to_keypress(&key_event);
            log::debug!(
                "key event processed: code={:?}, modifiers={:?}, scan=0x{:02X}, ascii=0x{:02X}",
                key_event.code,
                key_event.modifiers,
                key.scan_code,
                key.ascii_code
            );

            if key.scan_code == SCAN_CODE_F12 {
                // F12 - set command mode flag and don't buffer it
                self.command_mode_requested = true;
            } else {
                // Not F12 - buffer it for later retrieval by BIOS functions
                self.keyboard_buffer.push_back(key);
            }
        }
    }

    fn internal_read_key(&self) -> Option<KeyPress> {
        // Block until we get a key press
        loop {
            if let Ok(Event::Key(key_event)) = event::read() {
                if key_event.kind != KeyEventKind::Press {
                    continue;
                }
                let key_press = key_event_to_keypress(&key_event);
                log::debug!(
                    "key press (read_key): code={:?}, modifiers={:?}, scan=0x{:02X}, ascii=0x{:02X}",
                    key_event.code,
                    key_event.modifiers,
                    key_press.scan_code,
                    key_press.ascii_code
                );
                return Some(key_press);
            }
        }
    }

    fn internal_check_key(&self) -> Option<KeyPress> {
        // Check if a key is available without blocking.
        // Loop to consume and discard non-Press events (release/repeat) on Windows.
        loop {
            if !event::poll(Duration::from_millis(0)).unwrap_or(false) {
                return None;
            }
            let Ok(Event::Key(key_event)) = event::read() else {
                return None;
            };
            if key_event.kind != KeyEventKind::Press {
                continue;
            }
            let key_press = key_event_to_keypress(&key_event);
            log::debug!(
                "key press (check_key): code={:?}, modifiers={:?}, scan=0x{:02X}, ascii=0x{:02X}",
                key_event.code,
                key_event.modifiers,
                key_press.scan_code,
                key_press.ascii_code
            );
            return Some(key_press);
        }
    }
}

impl Default for TerminalKeyboard {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardInput for TerminalKeyboard {
    fn is_command_mode_requested(&self) -> bool {
        self.command_mode_requested
    }

    fn clear_command_mode_request(&mut self) {
        self.command_mode_requested = false;
    }

    fn process_event(&mut self, event: &dyn std::any::Any) -> bool {
        if let Some(event) = event.downcast_ref::<Event>() {
            self.process_crossterm_event(event.clone());
            true
        } else {
            false
        }
    }

    fn read_char(&mut self) -> Option<u8> {
        // Block until we get a key press (Press events only)
        loop {
            if let Ok(Event::Key(key_event)) = event::read() {
                if key_event.kind != KeyEventKind::Press {
                    continue;
                }
                if let Some(ch) = key_event_to_ascii(&key_event) {
                    return Some(translate_newline(ch));
                }
            }
        }
    }

    /// Check if a character is available and return it (non-blocking)
    /// Used by INT 21h, AH=06h (Direct Console I/O)
    fn check_char(&mut self) -> Option<u8> {
        loop {
            if !event::poll(Duration::from_millis(0)).unwrap_or(false) {
                return None;
            }
            let Ok(Event::Key(key_event)) = event::read() else {
                return None;
            };
            if key_event.kind != KeyEventKind::Press {
                continue;
            }
            if let Some(ch) = key_event_to_ascii(&key_event) {
                return Some(translate_newline(ch));
            }
            return None;
        }
    }

    /// Check if a character is available without consuming it
    /// Used by INT 21h, AH=0Bh (Check Input Status)
    fn has_char_available(&self) -> bool {
        event::poll(Duration::from_millis(0)).unwrap_or(false)
    }

    fn read_key(&mut self) -> Option<KeyPress> {
        // First check if we have a buffered key (from poll_for_command_key)
        if let Some(key) = self.keyboard_buffer.pop_front() {
            return Some(key);
        }

        // No buffered key, read from terminal (blocking)
        // Loop to handle F12 interception - keep blocking until we get a non-F12 key
        loop {
            let key = self.internal_read_key()?;
            // Intercept F12 for command mode
            if key.scan_code == SCAN_CODE_F12 {
                self.command_mode_requested = true;
                // Don't return F12 to the emulated program - loop to get next key
                continue;
            }
            return Some(key);
        }
    }

    fn check_key(&mut self) -> Option<KeyPress> {
        // First check if we have a buffered key (from poll_for_command_key)
        // Note: We remove it here to prevent infinite re-detection
        if let Some(key) = self.keyboard_buffer.pop_front() {
            return Some(key);
        }

        // No buffered key, check terminal (non-blocking)
        let key = self.internal_check_key()?;
        // Intercept F12 for command mode
        if key.scan_code == SCAN_CODE_F12 {
            self.command_mode_requested = true;
            // Return None so the emulated program doesn't see F12
            return None;
        }
        Some(key)
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

/// Translate Unix line endings to DOS
fn translate_newline(ch: u8) -> u8 {
    if ch == 0x0A {
        0x0D // LF -> CR
    } else {
        ch
    }
}

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
