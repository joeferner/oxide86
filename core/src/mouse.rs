//! Mouse input abstraction for platform-independent mouse support.
//!
//! This module provides a trait-based architecture for mouse input that works
//! across different platforms (terminal, GUI, WASM). The design follows the same
//! pattern as the keyboard input system.
//!
//! # Architecture
//!
//! - `MouseInput` trait: Platform-independent interface for mouse operations
//! - `MouseState`: Current mouse position and button states
//! - `NullMouse`: No-op implementation for platforms without mouse support
//!
//! # Usage
//!
//! Platform-specific implementations (e.g., `GuiMouse`, `TerminalMouse`) implement
//! the `MouseInput` trait and are used via `Box<dyn MouseInput>` in the BIOS.
//!
//! ```rust,ignore
//! let mouse: Box<dyn MouseInput> = Box::new(NullMouse::new());
//! let bios = Bios::new(keyboard, mouse);
//! ```

/// Current state of the mouse including position and button status.
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseState {
    /// Horizontal position (typically 0-639 for 640x200 resolution)
    pub x: u16,
    /// Vertical position (typically 0-199 for 640x200 resolution)
    pub y: u16,
    /// Left button pressed
    pub left_button: bool,
    /// Right button pressed
    pub right_button: bool,
    /// Middle button pressed
    pub middle_button: bool,
}

/// Platform-independent mouse input interface.
///
/// This trait provides methods for querying mouse state and motion, as well as
/// processing mouse events from platform-specific event loops.
///
/// # Object Safety
///
/// This trait is object-safe and designed to be used via `Box<dyn MouseInput>`.
/// All methods take `&self` or `&mut self` and have no generic parameters.
///
/// # Event Processing
///
/// Event processing methods (`process_cursor_moved`, `process_button`) have default
/// no-op implementations. Platform implementations override these as needed:
///
/// - `NullMouse`: Uses default no-ops
/// - `GuiMouse`: Overrides to update internal state from window events
/// - `TerminalMouse`: Overrides to update from terminal events
pub trait MouseInput {
    /// Returns the current mouse position and button states.
    ///
    /// # Returns
    ///
    /// `MouseState` containing current X/Y coordinates and button status.
    fn get_state(&self) -> MouseState;

    /// Returns accumulated mouse motion since last call and resets the counters.
    ///
    /// Motion is measured in "mickeys" (raw mouse movement units). The default
    /// ratio is 8 mickeys per pixel in DOS.
    ///
    /// # Returns
    ///
    /// Tuple of `(delta_x, delta_y)` in mickeys. The internal motion counters
    /// are reset to zero after this call.
    fn get_motion(&mut self) -> (i16, i16);

    /// Returns whether a mouse is present/available.
    ///
    /// # Returns
    ///
    /// `true` if mouse hardware is available, `false` otherwise.
    fn is_present(&self) -> bool;

    /// Process a cursor movement event (optional, default no-op).
    ///
    /// Platform-specific implementations override this to update internal state
    /// when the cursor moves. The coordinates are platform-specific:
    ///
    /// - Terminal: Character column/row (e.g., 0-79, 0-24)
    /// - GUI: Window pixel coordinates
    ///
    /// The implementation is responsible for converting to DOS coordinates.
    ///
    /// # Parameters
    ///
    /// - `_x`: Horizontal coordinate (platform-specific units)
    /// - `_y`: Vertical coordinate (platform-specific units)
    #[allow(unused_variables)]
    fn process_cursor_moved(&mut self, _x: f64, _y: f64) {
        // Default no-op implementation
    }

    /// Process relative mouse motion (optional, default no-op).
    ///
    /// Platform-specific implementations override this to handle raw mouse deltas.
    /// This is typically used in exclusive/locked cursor modes where absolute
    /// position tracking is disabled and only relative movement matters.
    ///
    /// The deltas are in platform-specific units (typically pixels). The
    /// implementation is responsible for scaling to DOS coordinates and mickeys.
    ///
    /// # Parameters
    ///
    /// - `_delta_x`: Horizontal movement delta (platform-specific units)
    /// - `_delta_y`: Vertical movement delta (platform-specific units)
    #[allow(unused_variables)]
    fn process_relative_motion(&mut self, _delta_x: f64, _delta_y: f64) {
        // Default no-op implementation
    }

    /// Process a button press/release event (optional, default no-op).
    ///
    /// Platform-specific implementations override this to update button state.
    ///
    /// # Parameters
    ///
    /// - `_button`: Button identifier (0=left, 1=right, 2=middle)
    /// - `_pressed`: `true` for press, `false` for release
    #[allow(unused_variables)]
    fn process_button(&mut self, _button: u8, _pressed: bool) {
        // Default no-op implementation
    }

    /// Update window dimensions for coordinate scaling (optional, default no-op).
    ///
    /// GUI implementations should override this to update coordinate conversion
    /// when the window is resized. This ensures mouse coordinates are properly
    /// scaled from window space to DOS coordinate space.
    ///
    /// # Parameters
    ///
    /// - `_width`: New window width in pixels
    /// - `_height`: New window height in pixels
    #[allow(unused_variables)]
    fn update_window_size(&mut self, _width: f64, _height: f64) {
        // Default no-op implementation
    }
}

/// Null mouse implementation for platforms without mouse support.
///
/// This implementation always reports no mouse present and provides no
/// state or motion data. Used as a fallback when mouse support is not
/// available or not needed.
///
/// # Example
///
/// ```rust,ignore
/// let mouse: Box<dyn MouseInput> = Box::new(NullMouse::new());
/// assert!(!mouse.is_present());
/// ```
pub struct NullMouse;

impl NullMouse {
    /// Creates a new null mouse implementation.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NullMouse {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseInput for NullMouse {
    fn get_state(&self) -> MouseState {
        MouseState::default()
    }

    fn get_motion(&mut self) -> (i16, i16) {
        (0, 0)
    }

    fn is_present(&self) -> bool {
        false
    }

    // Uses default trait implementations for process_cursor_moved and process_button
}
