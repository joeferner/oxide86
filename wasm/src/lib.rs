//! WebAssembly bindings for emu86 8086 emulator.
//!
//! This crate provides browser-based implementations of the emulator's
//! platform-independent traits (KeyboardInput, MouseInput, VideoController, DiskBackend).

pub mod web_keyboard;

// Re-export for convenience
pub use web_keyboard::WebKeyboard;
