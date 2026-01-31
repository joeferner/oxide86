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

/// Terminal-based mouse input for native CLI.
///
/// This struct manages mouse input from the terminal using crossterm events.
/// It tracks current position, button states, and accumulated motion deltas.
pub struct TerminalMouse {
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
            state: MouseState::default(),
            motion_x: 0,
            motion_y: 0,
            last_col: 0,
            last_row: 0,
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
        self.state
    }

    fn get_motion(&mut self) -> (i16, i16) {
        let motion = (self.motion_x, self.motion_y);
        // Reset motion counters after read
        self.motion_x = 0;
        self.motion_y = 0;
        motion
    }

    fn is_present(&self) -> bool {
        // Terminal mouse is always present (crossterm provides mouse events)
        true
    }

    fn process_cursor_moved(&mut self, x: f64, y: f64) {
        // Convert f64 coordinates to u16 (terminal column/row)
        let col = (x as u16).min(Self::TERMINAL_COLS - 1);
        let row = (y as u16).min(Self::TERMINAL_ROWS - 1);

        // Calculate motion delta in terminal coordinates
        let delta_col = (col as i16) - (self.last_col as i16);
        let delta_row = (row as i16) - (self.last_row as i16);

        // Accumulate motion in mickeys (8 mickeys per pixel, 8 pixels per char = 64 mickeys per char)
        // Motion in mickeys = delta in chars * pixels per char * mickeys per pixel
        self.motion_x = self
            .motion_x
            .saturating_add(delta_col * (Self::CHAR_WIDTH as i16));
        self.motion_y = self
            .motion_y
            .saturating_add(delta_row * (Self::CHAR_HEIGHT as i16));

        // Update last position
        self.last_col = col;
        self.last_row = row;

        // Convert to DOS coordinates and update state
        self.state.x = Self::terminal_col_to_dos_x(col);
        self.state.y = Self::terminal_row_to_dos_y(row);
    }

    fn process_button(&mut self, button: u8, pressed: bool) {
        match button {
            0 => self.state.left_button = pressed,
            1 => self.state.right_button = pressed,
            2 => self.state.middle_button = pressed,
            _ => {} // Ignore unknown buttons
        }
    }
}
