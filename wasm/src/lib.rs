//! WebAssembly bindings for emu86 8086 emulator.
//!
//! This crate provides browser-based implementations of the emulator's
//! platform-independent traits (KeyboardInput, MouseInput, VideoController, DiskBackend).

pub mod web_keyboard;
pub mod web_mouse;

// Re-export for convenience
pub use web_keyboard::WebKeyboard;
pub use web_mouse::WebMouse;
