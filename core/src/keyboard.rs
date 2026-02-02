//! Keyboard input abstraction for platform-independent keyboard handling.
//!
//! This module provides a trait-based interface for keyboard input that can be
//! implemented differently for native (CLI/GUI) and WebAssembly environments.

use crate::cpu::bios::KeyPress;

/// Platform-independent keyboard input trait.
///
/// This trait abstracts keyboard input operations, allowing different implementations
/// for terminal-based input, GUI event-driven input, and WebAssembly keyboard handling.
///
/// # Key Operations
///
/// The trait provides two types of operations:
///
/// 1. **Check operations** (`check_char`, `check_key`): Peek at the next key without removing it
/// 2. **Read operations** (`read_char`, `read_key`): Consume and return the next key
///
/// # Implementation Notes
///
/// - `read_*` methods should consume the key from the input buffer
/// - `check_*` methods should return the same key repeatedly until it's consumed by a `read_*` call
/// - All methods return `None` when no key is available
/// - Implementations should buffer input as needed to support both polling patterns
pub trait KeyboardInput {
    /// Read and consume the next ASCII character from the keyboard buffer.
    ///
    /// This method is typically used by simple character-based input routines.
    /// It returns the ASCII value of the key and removes it from the buffer.
    ///
    /// # Returns
    ///
    /// - `Some(ascii)` if a key is available
    /// - `None` if no key is available
    fn read_char(&mut self) -> Option<u8>;

    /// Check the next ASCII character without consuming it.
    ///
    /// This allows polling to see if a key is available without removing it
    /// from the buffer. Subsequent calls will return the same character until
    /// `read_char()` or `read_key()` is called.
    ///
    /// # Returns
    ///
    /// - `Some(ascii)` if a key is available
    /// - `None` if no key is available
    fn check_char(&mut self) -> Option<u8>;

    /// Check if a character is available in the keyboard buffer.
    ///
    /// This is a lighter-weight version of `check_char()` that only returns
    /// a boolean indicating availability without constructing the return value.
    ///
    /// # Returns
    ///
    /// - `true` if at least one key is available
    /// - `false` if the buffer is empty
    fn has_char_available(&self) -> bool;

    /// Read and consume the next key press with full scan code information.
    ///
    /// This method returns both the ASCII code and BIOS scan code, which is
    /// needed for function keys, arrow keys, and other extended keys that
    /// don't have simple ASCII representations.
    ///
    /// # Returns
    ///
    /// - `Some(KeyPress)` with scan_code and ascii_code if a key is available
    /// - `None` if no key is available
    fn read_key(&mut self) -> Option<KeyPress>;

    /// Check the next key press without consuming it.
    ///
    /// Similar to `check_char()`, but returns full `KeyPress` information
    /// including scan codes. The key remains in the buffer until consumed
    /// by `read_key()` or `read_char()`.
    ///
    /// # Returns
    ///
    /// - `Some(KeyPress)` with scan_code and ascii_code if a key is available
    /// - `None` if no key is available
    fn check_key(&mut self) -> Option<KeyPress>;

    // Platform-specific methods with default implementations

    /// Check if command mode (e.g., F12) has been requested.
    /// Default implementation returns false.
    fn is_command_mode_requested(&self) -> bool {
        false
    }

    /// Clear the command mode request flag.
    /// Default implementation does nothing.
    fn clear_command_mode_request(&mut self) {}

    /// Process a raw platform event (for CLI keyboard input).
    /// Default implementation does nothing and returns false.
    fn process_event(&mut self, _event: &dyn std::any::Any) -> bool {
        false
    }

    /// Convert a platform event to a KeyPress without buffering it.
    /// Used by GUI keyboards to extract KeyPress for firing keyboard interrupts.
    /// Default implementation returns None.
    fn event_to_keypress(&self, _event: &dyn std::any::Any) -> Option<KeyPress> {
        None
    }

    /// Update modifier key state from a platform-specific event.
    /// Used by GUI keyboards to track Shift, Ctrl, Alt state.
    /// Default implementation does nothing.
    fn update_modifiers(&mut self, _modifiers: &dyn std::any::Any) {}
}
