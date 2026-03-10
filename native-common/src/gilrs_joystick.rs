//! Gamepad input integration for the PC game port via the `gilrs` library.
//!
//! `GilrsJoystick` polls a `gilrs::Gilrs` context and translates gamepad events
//! into updates on a [`JoystickPortDevice`].
//!
//! # Axis mapping
//! - Left stick X/Y  → Joystick 1 X/Y (axes 0, 1)
//! - Right stick X/Y → Joystick 2 X/Y (axes 2, 3)
//!
//! # Button mapping
//! - South → Button 1, East → Button 2, North → Button 3, West → Button 4

use gilrs::{Axis, Button, EventType, Gilrs};
use oxide86_core::devices::game_port::GamePortDevice;

/// Translates gilrs gamepad events into [`JoystickPortDevice`] axis/button updates.
pub struct GilrsJoystick {
    gilrs: Gilrs,
    axes: [f32; 4],
    buttons: [bool; 4],
}

impl GilrsJoystick {
    /// Create a new `GilrsJoystick`.  Returns `None` if gilrs fails to initialise.
    pub fn new() -> Option<Self> {
        match Gilrs::new() {
            Ok(gilrs) => Some(Self {
                gilrs,
                axes: [0.0; 4],
                buttons: [false; 4],
            }),
            Err(e) => {
                log::warn!("Joystick: failed to initialize gilrs: {e}");
                None
            }
        }
    }

    /// Drain pending gilrs events and apply changes to `joystick`.
    /// Call this once per frame (or per emulation step) before stepping the CPU.
    pub fn poll(&mut self, joystick: &mut GamePortDevice) {
        let mut changed = false;

        while let Some(event) = self.gilrs.next_event() {
            match event.event {
                EventType::AxisChanged(axis, value, _) => {
                    if let Some(idx) = axis_index(axis) {
                        self.axes[idx] = value;
                        changed = true;
                    }
                }
                EventType::ButtonPressed(button, _) => {
                    if let Some(idx) = button_index(button) {
                        self.buttons[idx] = true;
                        changed = true;
                    }
                }
                EventType::ButtonReleased(button, _) => {
                    if let Some(idx) = button_index(button) {
                        self.buttons[idx] = false;
                        changed = true;
                    }
                }
                EventType::Connected => {
                    log::info!("Joystick: gamepad connected");
                }
                EventType::Disconnected => {
                    log::info!("Joystick: gamepad disconnected");
                    // Reset to neutral on disconnect
                    self.axes = [0.0; 4];
                    self.buttons = [false; 4];
                    changed = true;
                }
                _ => {}
            }
        }

        if changed {
            joystick.set_axes(
                normalize(self.axes[0]),
                normalize(self.axes[1]),
                normalize(self.axes[2]),
                normalize(self.axes[3]),
            );
            joystick.set_buttons(
                self.buttons[0],
                self.buttons[1],
                self.buttons[2],
                self.buttons[3],
            );
        }
    }
}

/// Normalize a gilrs axis value (−1.0..=1.0) to a gameport axis value (0..=255, center=128).
fn normalize(value: f32) -> u8 {
    ((value + 1.0) * 127.5).clamp(0.0, 255.0) as u8
}

fn axis_index(axis: Axis) -> Option<usize> {
    match axis {
        Axis::LeftStickX => Some(0),
        Axis::LeftStickY => Some(1),
        Axis::RightStickX => Some(2),
        Axis::RightStickY => Some(3),
        _ => None,
    }
}

fn button_index(button: Button) -> Option<usize> {
    match button {
        Button::South => Some(0),
        Button::East => Some(1),
        Button::North => Some(2),
        Button::West => Some(3),
        _ => None,
    }
}
