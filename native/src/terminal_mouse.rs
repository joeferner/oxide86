//! Terminal-based mouse input implementation for native CLI.
//!
//! This module provides a terminal-based mouse input implementation that:
//! - Implements the MouseInput trait for platform-independent mouse handling
//! - Uses crossterm for cross-platform terminal mouse events
//! - Converts terminal character coordinates to DOS screen coordinates
//! - Tracks mouse position, buttons, and motion deltas (mickeys)
//!
//! # Coordinate System
//!
//! Terminal coordinates are character-based (e.g., 80x25 for standard text mode):
//! - Column: 0-79 (horizontal character position)
//! - Row: 0-24 (vertical character position)
//!
//! DOS coordinates are pixel-based (640x200 for standard resolution):
//! - X: 0-639 (horizontal pixels)
//! - Y: 0-199 (vertical pixels)
//!
//! Conversion: Multiply terminal coords by 8 (character cell size in pixels)
//! - Example: Column 40, Row 12 → X=320, Y=96

use emu86_core::mouse::{MouseInput, MouseState};
use std::sync::{Arc, Mutex};

/// Shared internal state for TerminalMouse.
///
/// This allows multiple TerminalMouse instances to share the same state,
/// which is needed when attaching a serial mouse device that reads from
/// the same mouse input source.
#[derive(Debug, Default)]
struct SharedMouseState {
    /// Current mouse state (position and buttons)
    state: MouseState,
    /// Accumulated horizontal motion in mickeys since last read
    motion_x: i16,
    /// Accumulated vertical motion in mickeys since last read
    motion_y: i16,
    /// Last terminal column for delta calculation
    last_col: u16,
    /// Last terminal row for delta calculation
    last_row: u16,
    /// Whether we've received the first position update
    initialized: bool,
}

/// Terminal-based mouse input for native CLI.
///
/// This struct manages mouse input from the terminal using crossterm events.
/// It tracks current position, button states, and accumulated motion deltas.
///
/// Multiple instances can share the same state via the Arc<Mutex<...>> wrapper,
/// allowing both the BIOS and serial mouse device to read from the same input.
pub struct TerminalMouse {
    /// Shared state wrapped in Arc<Mutex<...>> for multi-owner access
    shared: Arc<Mutex<SharedMouseState>>,
}

impl TerminalMouse {
    /// Terminal dimensions (standard 80x25 text mode)
    const TERMINAL_COLS: u16 = 80;
    const TERMINAL_ROWS: u16 = 25;

    /// DOS screen dimensions (640x200 resolution)
    const DOS_WIDTH: u16 = 640;
    const DOS_HEIGHT: u16 = 200;

    /// Character cell size in pixels (DOS coords per terminal coord)
    const CHAR_WIDTH: u16 = 8;
    const CHAR_HEIGHT: u16 = 8;

    /// Create a new TerminalMouse instance.
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(SharedMouseState::default())),
        }
    }

    /// Create a new TerminalMouse instance that shares state with this one.
    ///
    /// This is useful for attaching a serial mouse device that needs to read
    /// from the same mouse input source as the BIOS.
    ///
    /// # Returns
    ///
    /// A new TerminalMouse instance that shares the same internal state.
    pub fn clone_shared(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Convert terminal column to DOS X coordinate.
    ///
    /// # Parameters
    ///
    /// - `col`: Terminal column (0-79)
    ///
    /// # Returns
    ///
    /// DOS X coordinate (0-639), clamped to valid range
    fn terminal_col_to_dos_x(col: u16) -> u16 {
        let x = col.saturating_mul(Self::CHAR_WIDTH);
        x.min(Self::DOS_WIDTH - 1)
    }

    /// Convert terminal row to DOS Y coordinate.
    ///
    /// # Parameters
    ///
    /// - `row`: Terminal row (0-24)
    ///
    /// # Returns
    ///
    /// DOS Y coordinate (0-199), clamped to valid range
    fn terminal_row_to_dos_y(row: u16) -> u16 {
        let y = row.saturating_mul(Self::CHAR_HEIGHT);
        y.min(Self::DOS_HEIGHT - 1)
    }
}

impl Default for TerminalMouse {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseInput for TerminalMouse {
    fn get_state(&self) -> MouseState {
        let shared = self.shared.lock().unwrap();
        shared.state
    }

    fn get_motion(&mut self) -> (i16, i16) {
        let mut shared = self.shared.lock().unwrap();
        let motion = (shared.motion_x, shared.motion_y);
        // Reset motion counters after read
        shared.motion_x = 0;
        shared.motion_y = 0;
        motion
    }

    fn is_present(&self) -> bool {
        // Terminal mouse is always present (crossterm provides mouse events)
        true
    }

    fn process_cursor_moved(&mut self, x: f64, y: f64) {
        let mut shared = self.shared.lock().unwrap();

        // Convert f64 coordinates to u16 (terminal column/row)
        let col = (x as u16).min(Self::TERMINAL_COLS - 1);
        let row = (y as u16).min(Self::TERMINAL_ROWS - 1);

        // On first movement, just initialize position without generating delta
        if !shared.initialized {
            shared.last_col = col;
            shared.last_row = row;
            shared.initialized = true;
            shared.state.x = Self::terminal_col_to_dos_x(col);
            shared.state.y = Self::terminal_row_to_dos_y(row);
            log::debug!("TerminalMouse: initialized at col={}, row={}", col, row);
            return;
        }

        // Calculate motion delta in terminal coordinates
        let delta_col = (col as i16) - (shared.last_col as i16);
        let delta_row = (row as i16) - (shared.last_row as i16);

        if delta_col != 0 || delta_row != 0 {
            log::debug!(
                "TerminalMouse: cursor moved to col={}, row={} (delta: {}, {})",
                col,
                row,
                delta_col,
                delta_row
            );
        }

        // Accumulate motion in mickeys
        // For text mode mouse, use 1:1 ratio (1 mickey = 1 character cell)
        // This matches what CTMOUSE expects in text mode
        shared.motion_x = shared.motion_x.saturating_add(delta_col);
        shared.motion_y = shared.motion_y.saturating_add(delta_row);

        // Update last position
        shared.last_col = col;
        shared.last_row = row;

        // Convert to DOS coordinates and update state
        shared.state.x = Self::terminal_col_to_dos_x(col);
        shared.state.y = Self::terminal_row_to_dos_y(row);
    }

    fn process_button(&mut self, button: u8, pressed: bool) {
        let mut shared = self.shared.lock().unwrap();
        match button {
            0 => shared.state.left_button = pressed,
            1 => shared.state.right_button = pressed,
            2 => shared.state.middle_button = pressed,
            _ => {} // Ignore unknown buttons
        }
    }
}
