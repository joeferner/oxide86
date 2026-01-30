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

use crate::bios::console::{self, SCAN_CODE_F12};

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
        while let Some(key) = console::check_key() {
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
}

impl Default for TerminalKeyboard {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardInput for TerminalKeyboard {
    fn read_char(&mut self) -> Option<u8> {
        console::read_char()
    }

    fn check_char(&mut self) -> Option<u8> {
        console::check_char()
    }

    fn has_char_available(&self) -> bool {
        console::has_char_available()
    }

    fn read_key(&mut self) -> Option<KeyPress> {
        // First check if we have a buffered key (from poll_for_command_key)
        if let Some(key) = self.keyboard_buffer.pop_front() {
            return Some(key);
        }

        // No buffered key, read from terminal (blocking)
        let key = console::read_key()?;
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
        let key = console::check_key()?;
        // Intercept F12 for command mode
        if key.scan_code == SCAN_CODE_F12 {
            self.command_mode_requested = true;
            // Return None so the emulated program doesn't see F12
            return None;
        }
        Some(key)
    }
}
