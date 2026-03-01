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

/// Translate Unix line endings to DOS
fn translate_newline(ch: u8) -> u8 {
    if ch == 0x0A {
        0x0D // LF -> CR
    } else {
        ch
    }
}


