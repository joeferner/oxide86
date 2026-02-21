//! Web-based joystick input using browser Gamepad API.
//!
//! This module provides a JoystickInput implementation for WebAssembly that receives
//! gamepad events from JavaScript and converts them to DOS-compatible joystick input.
//!
//! # Architecture
//!
//! - JavaScript polls navigator.getGamepads() in animation frame callback
//! - JavaScript calls handle_gamepad_axis and handle_gamepad_button methods
//! - Supports up to 2 joysticks (A and B) mapped to gamepad indices 0 and 1
//! - Axis values are normalized 0.0-1.0 (center = 0.5)
//! - Button states are boolean (pressed/released)
//!
//! # Usage
//!
//! ```rust,ignore
//! let joystick = WebJoystick::new();
//! let bios = Bios::new(keyboard, mouse, Box::new(joystick));
//! ```
//!
//! From JavaScript:
//! ```javascript
//! function pollGamepads() {
//!     const gamepads = navigator.getGamepads();
//!     if (gamepads[0]) {
//!         // Normalize -1..1 to 0..1
//!         const x = (gamepads[0].axes[0] + 1) / 2;
//!         const y = (gamepads[0].axes[1] + 1) / 2;
//!         computer.handle_gamepad_axis(0, 0, x); // Joystick A, X axis
//!         computer.handle_gamepad_axis(0, 1, y); // Joystick A, Y axis
//!         computer.handle_gamepad_button(0, 0, gamepads[0].buttons[0].pressed);
//!         computer.handle_gamepad_button(0, 1, gamepads[0].buttons[1].pressed);
//!     }
//! }
//! ```

use oxide86_core::joystick::{JoystickInput, JoystickState};
use std::cell::RefCell;
use std::rc::Rc;

/// Shared state for joystick input
#[derive(Debug, Clone, Default)]
struct SharedState {
    joysticks: [JoystickState; 2],
}

/// Web-based joystick input using browser Gamepad API.
///
/// JavaScript code polls navigator.getGamepads() and calls the inject methods
/// to update joystick state.
pub struct WebJoystick {
    state: Rc<RefCell<SharedState>>,
}

impl WebJoystick {
    /// Create a new WebJoystick.
    ///
    /// JavaScript should poll navigator.getGamepads() in requestAnimationFrame
    /// and call handle_gamepad_axis and handle_gamepad_button to update state.
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(SharedState::default())),
        }
    }

    /// Update joystick axis value (called from JavaScript)
    ///
    /// # Parameters
    /// - `joystick`: Joystick slot (0 = A, 1 = B)
    /// - `axis`: Axis number (0 = X, 1 = Y)
    /// - `value`: Normalized value 0.0-1.0 (center = 0.5)
    pub fn handle_gamepad_axis(&mut self, joystick: u8, axis: u8, value: f32) {
        let mut state = self.state.borrow_mut();
        if joystick < 2 && axis < 2 {
            match axis {
                0 => state.joysticks[joystick as usize].x = value.clamp(0.0, 1.0),
                1 => state.joysticks[joystick as usize].y = value.clamp(0.0, 1.0),
                _ => {}
            }
        }
    }

    /// Update joystick button state (called from JavaScript)
    ///
    /// # Parameters
    /// - `joystick`: Joystick slot (0 = A, 1 = B)
    /// - `button`: Button number (0 = button 1, 1 = button 2)
    /// - `pressed`: true if button is pressed, false if released
    pub fn handle_gamepad_button(&mut self, joystick: u8, button: u8, pressed: bool) {
        let mut state = self.state.borrow_mut();
        if joystick < 2 && button < 2 {
            match button {
                0 => state.joysticks[joystick as usize].button1 = pressed,
                1 => state.joysticks[joystick as usize].button2 = pressed,
                _ => {}
            }
        }
    }

    /// Set joystick connection state (called from JavaScript)
    ///
    /// # Parameters
    /// - `joystick`: Joystick slot (0 = A, 1 = B)
    /// - `connected`: true if gamepad is connected, false if disconnected
    pub fn gamepad_connected(&mut self, joystick: u8, connected: bool) {
        let mut state = self.state.borrow_mut();
        if joystick < 2 {
            state.joysticks[joystick as usize].connected = connected;
        }
    }
}

impl JoystickInput for WebJoystick {
    fn get_axis(&self, joystick: u8, axis: u8) -> f32 {
        let state = self.state.borrow();
        if joystick < 2 && axis < 2 {
            match axis {
                0 => state.joysticks[joystick as usize].x,
                1 => state.joysticks[joystick as usize].y,
                _ => 0.5,
            }
        } else {
            0.5 // centered
        }
    }

    fn get_button(&self, joystick: u8, button: u8) -> bool {
        let state = self.state.borrow();
        if joystick < 2 && button < 2 {
            match button {
                0 => state.joysticks[joystick as usize].button1,
                1 => state.joysticks[joystick as usize].button2,
                _ => false,
            }
        } else {
            false
        }
    }

    fn is_connected(&self, joystick: u8) -> bool {
        let state = self.state.borrow();
        if joystick < 2 {
            state.joysticks[joystick as usize].connected
        } else {
            false
        }
    }
}
