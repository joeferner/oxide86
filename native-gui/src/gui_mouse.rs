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
//! In text mode (80x25 characters), the mouse operates in character cell units:
//! - Each character is 8x16 pixels on screen (640x400 total)
//! - DOS mouse coordinates are 640x200 (each cell maps to 8x8 DOS pixels)
//! - Mouse position snaps to character cell boundaries
//!
//! Conversion:
//! - Window X (0-640) -> Column (0-79) -> DOS X (col * 8)
//! - Window Y (0-400) -> Row (0-24) -> DOS Y (row * 8)
//!
//! # Motion Tracking
//!
//! Mouse motion is tracked in mickeys (raw movement units):
//! - Moving 1 character cell horizontally = 8 mickeys (DOS_CHAR_WIDTH)
//! - Moving 1 character cell vertically = 8 mickeys (DOS_CHAR_HEIGHT)
//!
//! Motion accumulates between calls to get_motion() and is reset when retrieved.

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
    /// Last character column for delta calculation
    last_col: u16,
    /// Last character row for delta calculation
    last_row: u16,
    /// Whether we've received the first position update
    initialized: bool,
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
    /// Text mode dimensions (standard 80x25)
    const TEXT_COLS: u16 = 80;
    const TEXT_ROWS: u16 = 25;

    /// Character cell size in screen pixels
    const CHAR_WIDTH_PX: u16 = 8;
    const CHAR_HEIGHT_PX: u16 = 16;

    /// DOS mouse coordinate ranges (640x200)
    const DOS_MAX_X: u16 = 639;
    const DOS_MAX_Y: u16 = 199;

    /// Character cell size in DOS coordinates
    const DOS_CHAR_WIDTH: u16 = 8;
    const DOS_CHAR_HEIGHT: u16 = 8;

    /// Create a new GuiMouse instance.
    ///
    /// # Arguments
    ///
    /// * `window_width` - Initial window width in pixels
    /// * `window_height` - Initial window height in pixels
    ///
    /// The mouse position will be initialized on first movement.
    pub fn new(window_width: f64, window_height: f64) -> Self {
        Self {
            shared: Arc::new(Mutex::new(SharedMouseState {
                state: MouseState {
                    x: 0,
                    y: 0,
                    left_button: false,
                    right_button: false,
                    middle_button: false,
                },
                motion_x: 0,
                motion_y: 0,
                last_col: 0,
                last_row: 0,
                initialized: false,
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

    /// Convert window pixel coordinates to character column/row.
    ///
    /// # Arguments
    ///
    /// * `window_x` - X coordinate in window space
    /// * `window_y` - Y coordinate in window space
    /// * `window_width` - Current window width
    /// * `window_height` - Current window height
    ///
    /// # Returns
    ///
    /// Tuple of (col, row) clamped to valid text mode range.
    fn window_to_char_cell(
        window_x: f64,
        window_y: f64,
        window_width: f64,
        window_height: f64,
    ) -> (u16, u16) {
        // Scale window coordinates to screen pixel coordinates (640x400)
        let screen_width = (Self::TEXT_COLS * Self::CHAR_WIDTH_PX) as f64;
        let screen_height = (Self::TEXT_ROWS * Self::CHAR_HEIGHT_PX) as f64;

        let screen_x = (window_x / window_width) * screen_width;
        let screen_y = (window_y / window_height) * screen_height;

        // Convert screen pixels to character cells
        let col = (screen_x / Self::CHAR_WIDTH_PX as f64) as u16;
        let row = (screen_y / Self::CHAR_HEIGHT_PX as f64) as u16;

        // Clamp to valid range
        let col = col.min(Self::TEXT_COLS - 1);
        let row = row.min(Self::TEXT_ROWS - 1);

        (col, row)
    }

    /// Convert character column to DOS X coordinate.
    fn col_to_dos_x(col: u16) -> u16 {
        let x = col.saturating_mul(Self::DOS_CHAR_WIDTH);
        x.min(Self::DOS_MAX_X)
    }

    /// Convert character row to DOS Y coordinate.
    fn row_to_dos_y(row: u16) -> u16 {
        let y = row.saturating_mul(Self::DOS_CHAR_HEIGHT);
        y.min(Self::DOS_MAX_Y)
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

        // Convert window coordinates to character cell
        let (col, row) = Self::window_to_char_cell(x, y, state.window_width, state.window_height);

        // On first movement, just initialize position without generating delta
        if !state.initialized {
            state.last_col = col;
            state.last_row = row;
            state.initialized = true;
            state.state.x = Self::col_to_dos_x(col);
            state.state.y = Self::row_to_dos_y(row);
            log::debug!("GuiMouse: initialized at col={}, row={}", col, row);
            return;
        }

        // Calculate motion delta in character cells
        let delta_col = (col as i16) - (state.last_col as i16);
        let delta_row = (row as i16) - (state.last_row as i16);

        if delta_col != 0 || delta_row != 0 {
            log::debug!(
                "GuiMouse: cursor moved to col={}, row={} (delta: {}, {})",
                col,
                row,
                delta_col,
                delta_row
            );
        }

        // Accumulate motion in mickeys
        // Each character cell is 8x8 DOS pixels, so moving 1 cell = 8 mickeys
        let mickeys_x = delta_col * (Self::DOS_CHAR_WIDTH as i16);
        let mickeys_y = delta_row * (Self::DOS_CHAR_HEIGHT as i16);
        state.motion_x = state.motion_x.saturating_add(mickeys_x);
        state.motion_y = state.motion_y.saturating_add(mickeys_y);

        // Update last position
        state.last_col = col;
        state.last_row = row;

        // Convert to DOS coordinates and update state
        state.state.x = Self::col_to_dos_x(col);
        state.state.y = Self::row_to_dos_y(row);
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

    fn update_window_size(&mut self, width: f64, height: f64) {
        let mut state = self.shared.lock().unwrap();
        state.window_width = width;
        state.window_height = height;
    }
}
