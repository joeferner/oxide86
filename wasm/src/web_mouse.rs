//! Web-based mouse input using browser mouse events.
//!
//! This module provides a MouseInput implementation for WebAssembly that captures
//! mouse events from an HTML canvas element and converts them to DOS-compatible
//! mouse coordinates and button states.
//!
//! # Architecture
//!
//! - Attaches event listeners to an HTML canvas element
//! - Converts canvas pixel coordinates to DOS text mode coordinates (80x25)
//! - Tracks button states (left, right, middle)
//! - Accumulates mouse motion deltas in mickeys (8 mickeys per pixel)
//! - Stores closures to prevent JavaScript garbage collection
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
//! let canvas = get_canvas_element();
//! let mouse = WebMouse::new(&canvas)?;
//! let bios = Bios::new(keyboard, Box::new(mouse));
//! ```

use emu86_core::mouse::{MouseInput, MouseState};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, MouseEvent};

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
/// This implementation attaches event listeners to an HTML canvas element
/// and tracks mouse position and button states. The canvas dimensions are
/// used to scale mouse coordinates to DOS graphics mode (640x200).
pub struct WebMouse {
    /// Shared state between event handlers and MouseInput implementation
    state: Rc<RefCell<SharedState>>,
    /// Canvas width for coordinate scaling
    canvas_width: f64,
    /// Canvas height for coordinate scaling
    canvas_height: f64,
    /// Stored closure for mousemove events (prevents garbage collection)
    _mousemove_closure: Closure<dyn FnMut(MouseEvent)>,
    /// Stored closure for mousedown events (prevents garbage collection)
    _mousedown_closure: Closure<dyn FnMut(MouseEvent)>,
    /// Stored closure for mouseup events (prevents garbage collection)
    _mouseup_closure: Closure<dyn FnMut(MouseEvent)>,
}

impl WebMouse {
    /// Create a new WebMouse and attach event listeners to canvas.
    ///
    /// # Arguments
    ///
    /// * `canvas` - The HTML canvas element to attach mouse event listeners to
    ///
    /// # Returns
    ///
    /// A new WebMouse instance with event listeners attached, or a JsValue error
    /// if event listener attachment fails.
    ///
    /// # Coordinate Scaling
    ///
    /// The canvas dimensions are captured at creation time and used to scale
    /// mouse coordinates to DOS graphics resolution (640x200). If the canvas
    /// is resized later, the mouse will not automatically adjust.
    pub fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let state = Rc::new(RefCell::new(SharedState::default()));
        let canvas_width = canvas.width() as f64;
        let canvas_height = canvas.height() as f64;

        // Mousemove handler: update position and accumulate motion
        let state_move = state.clone();
        let mousemove_closure = Closure::wrap(Box::new(move |event: MouseEvent| {
            // Convert canvas coordinates to DOS graphics coordinates (640x200)
            let x = ((event.offset_x() as f64 / canvas_width) * 640.0).min(639.0) as u16;
            let y = ((event.offset_y() as f64 / canvas_height) * 200.0).min(199.0) as u16;

            let mut shared = state_move.borrow_mut();

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
        }) as Box<dyn FnMut(MouseEvent)>);

        // Mousedown handler: update button states
        let state_down = state.clone();
        let mousedown_closure = Closure::wrap(Box::new(move |event: MouseEvent| {
            let mut shared = state_down.borrow_mut();
            match event.button() {
                0 => shared.state.left_button = true,   // Left button
                1 => shared.state.middle_button = true, // Middle button
                2 => shared.state.right_button = true,  // Right button
                _ => {}
            }
        }) as Box<dyn FnMut(MouseEvent)>);

        // Mouseup handler: clear button states
        let state_up = state.clone();
        let mouseup_closure = Closure::wrap(Box::new(move |event: MouseEvent| {
            let mut shared = state_up.borrow_mut();
            match event.button() {
                0 => shared.state.left_button = false,   // Left button
                1 => shared.state.middle_button = false, // Middle button
                2 => shared.state.right_button = false,  // Right button
                _ => {}
            }
        }) as Box<dyn FnMut(MouseEvent)>);

        // Attach event listeners to canvas
        canvas.add_event_listener_with_callback(
            "mousemove",
            mousemove_closure.as_ref().unchecked_ref(),
        )?;
        canvas.add_event_listener_with_callback(
            "mousedown",
            mousedown_closure.as_ref().unchecked_ref(),
        )?;
        canvas.add_event_listener_with_callback(
            "mouseup",
            mouseup_closure.as_ref().unchecked_ref(),
        )?;

        log::info!(
            "WebMouse initialized with canvas dimensions {}x{}",
            canvas_width,
            canvas_height
        );

        Ok(Self {
            state,
            canvas_width,
            canvas_height,
            _mousemove_closure: mousemove_closure,
            _mousedown_closure: mousedown_closure,
            _mouseup_closure: mouseup_closure,
        })
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
