//! GUI keyboard input implementation for native GUI using winit.
//!
//! This module provides a GUI-based keyboard input implementation that:
//! - Implements the KeyboardInput trait for platform-independent keyboard handling
//! - Processes winit KeyboardInput events from the event loop
//! - Buffers keyboard input for retrieval by BIOS keyboard functions
//! - Converts winit KeyCode to 8086 scan codes and ASCII codes
//!
//! # Blocking vs Non-Blocking Behavior
//!
//! Unlike the terminal implementation which can block the thread waiting for input,
//! the GUI implementation is non-blocking because:
//! - We run on the main UI thread and blocking would freeze the window
//! - Input events arrive asynchronously from the winit event loop
//! - The emulator executes in steps between event loop iterations
//!
//! This means when a DOS program waits for input (INT 16h AH=00h), it will receive
//! `None` if no key is buffered, and will typically spin-loop calling the BIOS
//! function repeatedly until a key becomes available.

// Allow dead code warnings - this module will be used by GuiBios in phase 3.4
#![allow(dead_code)]

use emu86_core::cpu::bios::KeyPress;
use emu86_core::keyboard::KeyboardInput;
use std::collections::VecDeque;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// GUI keyboard input for native GUI using winit.
///
/// This struct manages keyboard input from winit events, buffering key presses
/// for retrieval by BIOS keyboard interrupt handlers.
pub struct GuiKeyboard {
    /// Buffer for keyboard input from winit events
    keyboard_buffer: VecDeque<KeyPress>,
}

impl GuiKeyboard {
    /// Create a new GuiKeyboard instance.
    pub fn new() -> Self {
        Self {
            keyboard_buffer: VecDeque::new(),
        }
    }

    /// Process a winit keyboard event and buffer the key press.
    ///
    /// This method should be called from the main event loop when a
    /// `WindowEvent::KeyboardInput` event is received.
    ///
    /// # Arguments
    ///
    /// * `event` - The keyboard event from winit
    ///
    /// # Behavior
    ///
    /// - Only processes key press events (ignores key release)
    /// - Converts the key to a KeyPress with scan code and ASCII code
    /// - Buffers the key for later retrieval via KeyboardInput methods
    pub fn process_event(&mut self, event: &KeyEvent) {
        // Only process key press events, ignore key release
        if event.state != ElementState::Pressed {
            return;
        }

        // Convert the winit key event to a KeyPress
        let key_press = key_event_to_keypress(event);

        // Buffer the key press
        self.keyboard_buffer.push_back(key_press);

        log::debug!(
            "Buffered key: scan_code=0x{:02X}, ascii_code=0x{:02X}",
            key_press.scan_code,
            key_press.ascii_code
        );
    }
}

impl Default for GuiKeyboard {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardInput for GuiKeyboard {
    /// Read and consume a character from the keyboard buffer.
    ///
    /// # Non-Blocking Behavior
    ///
    /// Unlike the terminal implementation, this is non-blocking in the GUI context.
    /// It returns `None` immediately if no key is buffered. DOS programs that wait
    /// for input will spin-loop calling BIOS functions until a key is available.
    fn read_char(&mut self) -> Option<u8> {
        // Pop the first available key and return its ASCII code
        self.keyboard_buffer.pop_front().map(|key| key.ascii_code)
    }

    /// Check for a character and consume it from the buffer.
    ///
    /// This is non-blocking and will return `None` if no key is available.
    fn check_char(&mut self) -> Option<u8> {
        // Pop the first available key and return its ASCII code
        // Note: DOS check operations typically consume the key
        self.keyboard_buffer.pop_front().map(|key| key.ascii_code)
    }

    fn has_char_available(&self) -> bool {
        !self.keyboard_buffer.is_empty()
    }

    /// Read and consume a key press from the buffer.
    ///
    /// # Non-Blocking Behavior
    ///
    /// Unlike the terminal implementation, this is non-blocking in the GUI context.
    /// It returns `None` immediately if no key is buffered. DOS programs that wait
    /// for input will spin-loop calling BIOS functions until a key is available.
    fn read_key(&mut self) -> Option<KeyPress> {
        // Pop and return the first available key
        self.keyboard_buffer.pop_front()
    }

    /// Check for a key press and consume it from the buffer.
    ///
    /// This is non-blocking and will return `None` if no key is available.
    fn check_key(&mut self) -> Option<KeyPress> {
        // Pop and return the first available key
        // Note: DOS check operations typically consume the key
        self.keyboard_buffer.pop_front()
    }
}

/// Convert winit KeyEvent to KeyPress with scan code and ASCII code
fn key_event_to_keypress(key_event: &KeyEvent) -> KeyPress {
    let scan_code = if let PhysicalKey::Code(key_code) = key_event.physical_key {
        key_code_to_scan_code(&key_code)
    } else {
        0x00
    };

    // Extract ASCII from the text field if available (this handles shift, etc.)
    let ascii_code = key_event
        .text
        .as_ref()
        .and_then(|text| text.chars().next())
        .map(|c| c as u8)
        .unwrap_or_else(|| key_code_to_ascii(&key_event.physical_key));

    KeyPress {
        scan_code,
        ascii_code,
    }
}

/// Map winit KeyCode to 8086 scan code
fn key_code_to_scan_code(code: &KeyCode) -> u8 {
    match code {
        // Row 1 - Number keys
        KeyCode::Escape => 0x01,
        KeyCode::Digit1 => 0x02,
        KeyCode::Digit2 => 0x03,
        KeyCode::Digit3 => 0x04,
        KeyCode::Digit4 => 0x05,
        KeyCode::Digit5 => 0x06,
        KeyCode::Digit6 => 0x07,
        KeyCode::Digit7 => 0x08,
        KeyCode::Digit8 => 0x09,
        KeyCode::Digit9 => 0x0A,
        KeyCode::Digit0 => 0x0B,
        KeyCode::Minus => 0x0C,
        KeyCode::Equal => 0x0D,
        KeyCode::Backspace => 0x0E,

        // Row 2 - Q row
        KeyCode::Tab => 0x0F,
        KeyCode::KeyQ => 0x10,
        KeyCode::KeyW => 0x11,
        KeyCode::KeyE => 0x12,
        KeyCode::KeyR => 0x13,
        KeyCode::KeyT => 0x14,
        KeyCode::KeyY => 0x15,
        KeyCode::KeyU => 0x16,
        KeyCode::KeyI => 0x17,
        KeyCode::KeyO => 0x18,
        KeyCode::KeyP => 0x19,
        KeyCode::BracketLeft => 0x1A,
        KeyCode::BracketRight => 0x1B,
        KeyCode::Enter => 0x1C,

        // Row 3 - A row
        KeyCode::KeyA => 0x1E,
        KeyCode::KeyS => 0x1F,
        KeyCode::KeyD => 0x20,
        KeyCode::KeyF => 0x21,
        KeyCode::KeyG => 0x22,
        KeyCode::KeyH => 0x23,
        KeyCode::KeyJ => 0x24,
        KeyCode::KeyK => 0x25,
        KeyCode::KeyL => 0x26,
        KeyCode::Semicolon => 0x27,
        KeyCode::Quote => 0x28,
        KeyCode::Backquote => 0x29,

        // Row 4 - Z row
        KeyCode::Backslash => 0x2B,
        KeyCode::KeyZ => 0x2C,
        KeyCode::KeyX => 0x2D,
        KeyCode::KeyC => 0x2E,
        KeyCode::KeyV => 0x2F,
        KeyCode::KeyB => 0x30,
        KeyCode::KeyN => 0x31,
        KeyCode::KeyM => 0x32,
        KeyCode::Comma => 0x33,
        KeyCode::Period => 0x34,
        KeyCode::Slash => 0x35,

        // Space and modifiers
        KeyCode::Space => 0x39,
        KeyCode::CapsLock => 0x3A,

        // Function keys
        KeyCode::F1 => 0x3B,
        KeyCode::F2 => 0x3C,
        KeyCode::F3 => 0x3D,
        KeyCode::F4 => 0x3E,
        KeyCode::F5 => 0x3F,
        KeyCode::F6 => 0x40,
        KeyCode::F7 => 0x41,
        KeyCode::F8 => 0x42,
        KeyCode::F9 => 0x43,
        KeyCode::F10 => 0x44,
        KeyCode::F11 => 0x85,
        KeyCode::F12 => 0x86,

        // Navigation keys
        KeyCode::Home => 0x47,
        KeyCode::ArrowUp => 0x48,
        KeyCode::PageUp => 0x49,
        KeyCode::ArrowLeft => 0x4B,
        KeyCode::ArrowRight => 0x4D,
        KeyCode::End => 0x4F,
        KeyCode::ArrowDown => 0x50,
        KeyCode::PageDown => 0x51,
        KeyCode::Insert => 0x52,
        KeyCode::Delete => 0x53,

        // Numpad keys
        KeyCode::NumLock => 0x45,
        KeyCode::ScrollLock => 0x46,
        KeyCode::Numpad7 => 0x47, // Home
        KeyCode::Numpad8 => 0x48, // Up
        KeyCode::Numpad9 => 0x49, // PgUp
        KeyCode::NumpadSubtract => 0x4A,
        KeyCode::Numpad4 => 0x4B, // Left
        KeyCode::Numpad5 => 0x4C,
        KeyCode::Numpad6 => 0x4D, // Right
        KeyCode::NumpadAdd => 0x4E,
        KeyCode::Numpad1 => 0x4F,       // End
        KeyCode::Numpad2 => 0x50,       // Down
        KeyCode::Numpad3 => 0x51,       // PgDn
        KeyCode::Numpad0 => 0x52,       // Insert
        KeyCode::NumpadDecimal => 0x53, // Delete

        _ => 0x00,
    }
}

/// Map winit PhysicalKey to ASCII code for special keys
fn key_code_to_ascii(key: &PhysicalKey) -> u8 {
    match key {
        PhysicalKey::Code(code) => match code {
            KeyCode::Enter => 0x0D,     // CR
            KeyCode::Backspace => 0x08, // BS
            KeyCode::Tab => 0x09,       // TAB
            KeyCode::Escape => 0x1B,    // ESC
            KeyCode::Delete => 0x7F,    // DEL
            KeyCode::Space => 0x20,     // SPACE
            _ => 0x00,
        },
        _ => 0x00,
    }
}
