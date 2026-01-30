//! Terminal-based keyboard input implementation for native CLI.
//!
//! This module provides a terminal-based keyboard input implementation that:
//! - Implements the KeyboardInput trait for platform-independent keyboard handling
//! - Handles F12 key interception for command mode (CLI-specific feature)
//! - Buffers keyboard input during polling to prevent key loss
//! - Uses crossterm for cross-platform terminal keyboard access

use emu86_core::cpu::bios::KeyPress;
use emu86_core::keyboard::KeyboardInput;
use std::collections::VecDeque;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

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

    /// Check if command mode has been requested via F12.
    ///
    /// This is a CLI-specific feature that allows the user to interrupt
    /// the emulation and enter a command prompt by pressing F12.
    pub fn is_command_mode_requested(&self) -> bool {
        self.command_mode_requested
    }

    /// Clear the command mode request flag.
    ///
    /// This should be called after handling the command mode request.
    pub fn clear_command_mode_request(&mut self) {
        self.command_mode_requested = false;
    }

    /// Poll for F12 key press without blocking.
    ///
    /// This method is called from the main loop to detect command mode requests
    /// even when the emulated program doesn't call keyboard BIOS functions.
    /// Keys other than F12 are buffered for later retrieval by BIOS functions.
    ///
    /// # Behavior
    ///
    /// - Drains all available keys from the terminal
    /// - F12 sets the command mode flag and stops processing
    /// - Other keys are buffered for later retrieval via KeyboardInput methods
    pub fn poll_for_command_key(&mut self) {
        // Drain all available keys from the terminal
        while let Some(key) = self.internal_check_key() {
            if key.scan_code == SCAN_CODE_F12 {
                // F12 - set command mode flag and don't buffer it
                self.command_mode_requested = true;
                break; // Stop processing once F12 is detected
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
                let key_press = key_event_to_keypress(&key_event);
                log::debug!("key press (read_key): 0x{:02X}", key_press.scan_code);
                return Some(key_press);
            }
        }
    }

    fn internal_check_key(&self) -> Option<KeyPress> {
        // Check if a key is available without blocking
        if event::poll(Duration::from_millis(0)).unwrap_or(false)
            && let Ok(Event::Key(key_event)) = event::read()
        {
            let key_press = key_event_to_keypress(&key_event);
            log::debug!("key press (check_key): 0x{:02X}", key_press.scan_code);
            return Some(key_press);
        }
        None
    }
}

impl Default for TerminalKeyboard {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardInput for TerminalKeyboard {
    fn read_char(&mut self) -> Option<u8> {
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
    fn check_char(&mut self) -> Option<u8> {
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
    fn has_char_available(&self) -> bool {
        event::poll(Duration::from_millis(0)).unwrap_or(false)
    }

    fn read_key(&mut self) -> Option<KeyPress> {
        // First check if we have a buffered key (from poll_for_command_key)
        if let Some(key) = self.keyboard_buffer.pop_front() {
            return Some(key);
        }

        // No buffered key, read from terminal (blocking)
        let key = self.internal_read_key()?;
        // Intercept F12 for command mode
        if key.scan_code == SCAN_CODE_F12 {
            self.command_mode_requested = true;
            // Return None so the emulated program doesn't see F12
            return None;
        }
        Some(key)
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
fn key_event_to_keypress(key_event: &KeyEvent) -> KeyPress {
    let scan_code = key_code_to_scan_code(&key_event.code);
    let ascii_code = key_event_to_ascii(key_event).unwrap_or(0x00);

    KeyPress {
        scan_code,
        ascii_code,
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
