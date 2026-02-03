//! Web-based mouse input using browser mouse events.
//!
//! This module provides a MouseInput implementation for WebAssembly that receives
//! mouse events from JavaScript and converts them to DOS-compatible mouse coordinates
//! and button states.
//!
//! # Architecture
//!
//! - Event listeners are attached in JavaScript code
//! - JavaScript calls inject_mouse_move and inject_mouse_button methods
//! - Converts canvas pixel coordinates to DOS text mode coordinates (80x25)
//! - Tracks button states (left, right, middle)
//! - Accumulates mouse motion deltas in mickeys (8 mickeys per pixel)
//!
//! # Coordinate System
//!
//! Canvas coordinates are scaled to DOS graphics resolution (640x200):
//! - Canvas pixel coordinates → 640x200 virtual coordinates
//! - DOS text mode uses 80x25 characters, so each char = 8x8 pixels
//!
//! # Usage
//!
//! ```rust,ignore
//! let mouse = WebMouse::new(canvas_width, canvas_height)?;
//! let bios = Bios::new(keyboard, Box::new(mouse));
//! ```

use emu86_core::mouse::{MouseInput, MouseState};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

/// Shared state between WebMouse and event closures.
#[derive(Debug, Clone, Default)]
struct SharedState {
    /// Current mouse state
    state: MouseState,
    /// Accumulated horizontal motion in mickeys (reset on get_motion call)
    motion_x: i16,
    /// Accumulated vertical motion in mickeys (reset on get_motion call)
    motion_y: i16,
    /// Previous X position for delta calculation
    prev_x: u16,
    /// Previous Y position for delta calculation
    prev_y: u16,
}

/// Web-based mouse input using browser mouse events.
///
/// Event listeners are attached in JavaScript, which calls inject methods.
/// The canvas dimensions are used to scale mouse coordinates to DOS graphics mode (640x200).
pub struct WebMouse {
    /// Shared state for mouse position and buttons
    state: Rc<RefCell<SharedState>>,
    /// Canvas width for coordinate scaling
    canvas_width: f64,
    /// Canvas height for coordinate scaling
    canvas_height: f64,
}

impl WebMouse {
    /// Create a new WebMouse with the given canvas dimensions.
    /// Event listeners should be attached in JavaScript code.
    ///
    /// # Arguments
    ///
    /// * `canvas_width` - Canvas width in pixels for coordinate scaling
    /// * `canvas_height` - Canvas height in pixels for coordinate scaling
    ///
    /// # Returns
    ///
    /// A new WebMouse instance ready to receive events from JavaScript.
    ///
    /// # Coordinate Scaling
    ///
    /// Mouse coordinates from JavaScript are scaled from canvas pixels to
    /// DOS graphics resolution (640x200). Use update_window_size if the canvas is resized.
    pub fn new(canvas_width: f64, canvas_height: f64) -> Result<Self, JsValue> {
        log::info!(
            "WebMouse initialized with canvas dimensions {}x{}",
            canvas_width,
            canvas_height
        );

        Ok(Self {
            state: Rc::new(RefCell::new(SharedState::default())),
            canvas_width,
            canvas_height,
        })
    }

    /// Inject a mouse move event from JavaScript.
    ///
    /// # Arguments
    ///
    /// * `offset_x` - Mouse X coordinate relative to canvas (in canvas pixels)
    /// * `offset_y` - Mouse Y coordinate relative to canvas (in canvas pixels)
    pub fn inject_mouse_move(&mut self, offset_x: f64, offset_y: f64) {
        // Convert canvas coordinates to DOS graphics coordinates (640x200)
        let x = ((offset_x / self.canvas_width) * 640.0).min(639.0) as u16;
        let y = ((offset_y / self.canvas_height) * 200.0).min(199.0) as u16;

        let mut shared = self.state.borrow_mut();

        // Calculate motion delta in pixels
        let delta_x = x as i16 - shared.prev_x as i16;
        let delta_y = y as i16 - shared.prev_y as i16;

        // Convert to mickeys (8 mickeys per pixel)
        shared.motion_x = shared.motion_x.saturating_add(delta_x * 8);
        shared.motion_y = shared.motion_y.saturating_add(delta_y * 8);

        // Update position
        shared.state.x = x;
        shared.state.y = y;
        shared.prev_x = x;
        shared.prev_y = y;
    }

    /// Inject a mouse button event from JavaScript.
    ///
    /// # Arguments
    ///
    /// * `button` - Button number (0=left, 1=middle, 2=right)
    /// * `pressed` - true for mousedown, false for mouseup
    pub fn inject_mouse_button(&mut self, button: u8, pressed: bool) {
        let mut shared = self.state.borrow_mut();
        match button {
            0 => shared.state.left_button = pressed,   // Left button
            1 => shared.state.middle_button = pressed, // Middle button
            2 => shared.state.right_button = pressed,  // Right button
            _ => {}
        }
    }
}

impl MouseInput for WebMouse {
    fn get_state(&self) -> MouseState {
        self.state.borrow().state
    }

    fn get_motion(&mut self) -> (i16, i16) {
        let mut shared = self.state.borrow_mut();
        let motion_x = shared.motion_x;
        let motion_y = shared.motion_y;

        // Reset motion counters
        shared.motion_x = 0;
        shared.motion_y = 0;

        (motion_x, motion_y)
    }

    fn is_present(&self) -> bool {
        // Mouse is always present in web environment
        true
    }

    fn update_window_size(&mut self, width: f64, height: f64) {
        // Update scaling dimensions if canvas is resized
        self.canvas_width = width;
        self.canvas_height = height;
        log::info!("WebMouse canvas dimensions updated to {}x{}", width, height);
    }
}
