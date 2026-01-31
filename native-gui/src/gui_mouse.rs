//! GUI mouse input implementation for native GUI using winit.
//!
//! This module provides a GUI-based mouse input implementation that:
//! - Implements the MouseInput trait for platform-independent mouse handling
//! - Processes winit mouse events from the event loop
//! - Converts window coordinates to DOS mouse coordinates (640x200 by default)
//! - Tracks button states and accumulates motion deltas (mickeys)
//!
//! # Coordinate System
//!
//! DOS mouse coordinates are typically 640x200 (standard CGA/EGA graphics mode),
//! even though the GUI window may be 640x400 (text mode). The implementation
//! scales window coordinates appropriately:
//!
//! - Window X (0-640) -> DOS X (0-639)
//! - Window Y (0-400) -> DOS Y (0-199)
//!
//! # Motion Tracking
//!
//! Mouse motion is tracked in "mickeys" (raw movement units). The default DOS
//! ratio is 8 mickeys per pixel. Motion accumulates between calls to get_motion()
//! and is reset when retrieved.

use emu86_core::mouse::{MouseInput, MouseState};
use std::sync::{Arc, Mutex};

/// Shared internal state for GuiMouse.
///
/// This allows multiple GuiMouse instances to share the same state,
/// which is needed when attaching a serial mouse device that reads from
/// the same mouse input source.
#[derive(Debug)]
struct SharedMouseState {
    /// Current mouse state (position and button states)
    state: MouseState,
    /// Accumulated horizontal motion in mickeys since last read
    motion_x: i16,
    /// Accumulated vertical motion in mickeys since last read
    motion_y: i16,
    /// Last raw X position for delta calculation
    last_x: f64,
    /// Last raw Y position for delta calculation
    last_y: f64,
    /// Window width for coordinate scaling
    window_width: f64,
    /// Window height for coordinate scaling
    window_height: f64,
}

/// GUI mouse input for native GUI using winit.
///
/// This struct manages mouse input from winit events, tracking position,
/// button states, and motion deltas for retrieval by BIOS mouse interrupt
/// handlers (INT 33h).
///
/// Multiple instances can share the same state via the Arc<Mutex<...>> wrapper,
/// allowing both the BIOS and serial mouse device to read from the same input.
pub struct GuiMouse {
    /// Shared state wrapped in Arc<Mutex<...>> for multi-owner access
    shared: Arc<Mutex<SharedMouseState>>,
}

impl GuiMouse {
    /// DOS mouse coordinate ranges (standard CGA/EGA graphics mode)
    const DOS_MAX_X: u16 = 639;
    const DOS_MAX_Y: u16 = 199;

    /// Default mickeys-per-pixel ratio (DOS standard)
    const MICKEYS_PER_PIXEL: i16 = 8;

    /// Create a new GuiMouse instance.
    ///
    /// # Arguments
    ///
    /// * `window_width` - Initial window width in pixels
    /// * `window_height` - Initial window height in pixels
    ///
    /// The mouse is initialized at the center of the DOS coordinate space (320, 100).
    pub fn new(window_width: f64, window_height: f64) -> Self {
        Self {
            shared: Arc::new(Mutex::new(SharedMouseState {
                state: MouseState {
                    x: 320, // Center X
                    y: 100, // Center Y
                    left_button: false,
                    right_button: false,
                    middle_button: false,
                },
                motion_x: 0,
                motion_y: 0,
                last_x: window_width / 2.0,
                last_y: window_height / 2.0,
                window_width,
                window_height,
            })),
        }
    }

    /// Create a new GuiMouse instance that shares state with this one.
    ///
    /// This is useful for attaching a serial mouse device that needs to read
    /// from the same mouse input as the main GUI mouse.
    ///
    /// # Returns
    ///
    /// A new GuiMouse instance that shares the same internal state.
    pub fn clone_shared(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Update window dimensions for coordinate scaling.
    ///
    /// This should be called when the window is resized to ensure
    /// proper coordinate conversion from window space to DOS space.
    ///
    /// # Arguments
    ///
    /// * `width` - New window width in pixels
    /// * `height` - New window height in pixels
    #[allow(dead_code)]
    pub fn update_window_size(&mut self, width: f64, height: f64) {
        let mut state = self.shared.lock().unwrap();
        state.window_width = width;
        state.window_height = height;
    }

    /// Convert window coordinates to DOS mouse coordinates.
    ///
    /// # Arguments
    ///
    /// * `window_x` - X coordinate in window space (0..window_width)
    /// * `window_y` - Y coordinate in window space (0..window_height)
    /// * `window_width` - Current window width
    /// * `window_height` - Current window height
    ///
    /// # Returns
    ///
    /// Tuple of (dos_x, dos_y) clamped to valid DOS coordinate range.
    fn window_to_dos_coords(
        window_x: f64,
        window_y: f64,
        window_width: f64,
        window_height: f64,
    ) -> (u16, u16) {
        // Scale window coordinates to DOS coordinate space
        let dos_x = ((window_x / window_width) * (Self::DOS_MAX_X as f64 + 1.0)) as i32;
        let dos_y = ((window_y / window_height) * (Self::DOS_MAX_Y as f64 + 1.0)) as i32;

        // Clamp to valid range
        let dos_x = dos_x.clamp(0, Self::DOS_MAX_X as i32) as u16;
        let dos_y = dos_y.clamp(0, Self::DOS_MAX_Y as i32) as u16;

        (dos_x, dos_y)
    }
}

impl MouseInput for GuiMouse {
    fn get_state(&self) -> MouseState {
        let state = self.shared.lock().unwrap();
        state.state
    }

    fn get_motion(&mut self) -> (i16, i16) {
        let mut state = self.shared.lock().unwrap();
        let motion = (state.motion_x, state.motion_y);
        // Reset motion counters after reading
        state.motion_x = 0;
        state.motion_y = 0;
        motion
    }

    fn is_present(&self) -> bool {
        true
    }

    fn process_cursor_moved(&mut self, x: f64, y: f64) {
        let mut state = self.shared.lock().unwrap();

        // Calculate delta in window coordinates
        let delta_x = x - state.last_x;
        let delta_y = y - state.last_y;

        // Convert delta to mickeys (scaled by mickeys-per-pixel ratio)
        // Note: We use window coordinates for delta calculation to get smooth motion
        let delta_mickeys_x = (delta_x * Self::MICKEYS_PER_PIXEL as f64) as i16;
        let delta_mickeys_y = (delta_y * Self::MICKEYS_PER_PIXEL as f64) as i16;

        // Accumulate motion
        state.motion_x = state.motion_x.saturating_add(delta_mickeys_x);
        state.motion_y = state.motion_y.saturating_add(delta_mickeys_y);

        // Update last position
        state.last_x = x;
        state.last_y = y;

        // Update DOS position
        let (dos_x, dos_y) =
            Self::window_to_dos_coords(x, y, state.window_width, state.window_height);
        state.state.x = dos_x;
        state.state.y = dos_y;
    }

    fn process_button(&mut self, button: u8, pressed: bool) {
        let mut state = self.shared.lock().unwrap();
        match button {
            0 => state.state.left_button = pressed,
            1 => state.state.right_button = pressed,
            2 => state.state.middle_button = pressed,
            _ => {} // Ignore unknown buttons
        }
    }
}
