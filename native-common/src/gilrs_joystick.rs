/// Joystick implementation using gilrs library (native platforms only)
///
/// Maps connected gamepads to joystick slots A and B:
/// - First connected gamepad → slot 0 (Joystick A)
/// - Second connected gamepad → slot 1 (Joystick B)
///
/// Axis mapping:
/// - LeftStickX / LeftStickY → X/Y axes
/// - South button (A/Cross) → button 1
/// - East button (B/Circle) → button 2
use emu86_core::joystick::{JoystickInput, JoystickState};
use gilrs::{Axis, Button, Event, EventType, Gilrs};
use std::sync::{Arc, Mutex};

/// Joystick implementation using gilrs for gamepad input
pub struct GilrsJoystick {
    state: Arc<Mutex<GilrsJoystickState>>,
    gilrs: Gilrs,
}

pub struct GilrsJoystickState {
    joysticks: [JoystickState; 2],
    // Map gilrs gamepad IDs to slot indices (0 or 1)
    gamepad_to_slot: std::collections::HashMap<gilrs::GamepadId, usize>,
}

impl GilrsJoystick {
    pub fn new() -> Self {
        let gilrs = match Gilrs::new() {
            Ok(gilrs) => gilrs,
            Err(e) => {
                log::warn!("Failed to initialize gamepad support: {}", e);
                log::warn!("Continuing without gamepad support");
                // Create a dummy GilrsJoystick with no gamepads
                // We can't create a Gilrs instance, so we'll have to handle this gracefully
                return Self {
                    state: Arc::new(Mutex::new(GilrsJoystickState {
                        joysticks: [JoystickState::default(); 2],
                        gamepad_to_slot: std::collections::HashMap::new(),
                    })),
                    // Create a new Gilrs that might fail - we'll just return early if needed
                    gilrs: Gilrs::new()
                        .unwrap_or_else(|_| panic!("Unable to initialize gilrs even for fallback")),
                };
            }
        };

        let mut state = GilrsJoystickState {
            joysticks: [JoystickState::default(); 2],
            gamepad_to_slot: std::collections::HashMap::new(),
        };

        // Detect already-connected gamepads
        let mut slot = 0;
        for (id, gamepad) in gilrs.gamepads() {
            if slot >= 2 {
                break;
            }
            if gamepad.is_connected() {
                state.gamepad_to_slot.insert(id, slot);
                state.joysticks[slot].connected = true;
                log::info!(
                    "Gamepad '{}' detected as joystick {}",
                    gamepad.name(),
                    if slot == 0 { "A" } else { "B" }
                );
                slot += 1;
            }
        }

        Self {
            state: Arc::new(Mutex::new(state)),
            gilrs,
        }
    }

    /// Poll gilrs for events and update joystick state
    /// Should be called periodically (e.g., each frame for GUI, or in game loop for CLI)
    pub fn poll(&mut self) {
        while let Some(Event { id, event, .. }) = self.gilrs.next_event() {
            let mut state = self.state.lock().unwrap();

            // Get or assign slot for this gamepad
            let slot = if let Some(&slot) = state.gamepad_to_slot.get(&id) {
                slot
            } else {
                // New gamepad connected - assign to first free slot
                let free_slot = if !state.joysticks[0].connected {
                    Some(0)
                } else if !state.joysticks[1].connected {
                    Some(1)
                } else {
                    None
                };

                if let Some(slot) = free_slot {
                    state.gamepad_to_slot.insert(id, slot);
                    state.joysticks[slot].connected = true;
                    let gamepad = self.gilrs.gamepad(id);
                    log::info!(
                        "Gamepad '{}' connected as joystick {}",
                        gamepad.name(),
                        if slot == 0 { "A" } else { "B" }
                    );
                    slot
                } else {
                    continue; // No free slots
                }
            };

            match event {
                EventType::ButtonPressed(button, _) => {
                    match button {
                        Button::South => state.joysticks[slot].button1 = true, // A/Cross
                        Button::East => state.joysticks[slot].button2 = true,  // B/Circle
                        _ => {}
                    }
                }
                EventType::ButtonReleased(button, _) => match button {
                    Button::South => state.joysticks[slot].button1 = false,
                    Button::East => state.joysticks[slot].button2 = false,
                    _ => {}
                },
                EventType::AxisChanged(axis, value, _) => {
                    // gilrs axes are -1.0 to 1.0, convert to 0.0 to 1.0
                    let normalized = (value + 1.0) / 2.0;
                    match axis {
                        Axis::LeftStickX => state.joysticks[slot].x = normalized,
                        Axis::LeftStickY => state.joysticks[slot].y = normalized,
                        _ => {}
                    }
                }
                EventType::Disconnected => {
                    state.joysticks[slot].connected = false;
                    state.gamepad_to_slot.remove(&id);
                    log::info!(
                        "Gamepad disconnected from joystick {}",
                        if slot == 0 { "A" } else { "B" }
                    );
                }
                _ => {}
            }
        }
    }

    /// Get a clone of the state for sharing with the JoystickInput trait
    pub fn clone_state(&self) -> Arc<Mutex<GilrsJoystickState>> {
        Arc::clone(&self.state)
    }
}

/// Wrapper for JoystickInput trait implementation
pub struct GilrsJoystickInput {
    state: Arc<Mutex<GilrsJoystickState>>,
}

impl GilrsJoystickInput {
    pub fn new(state: Arc<Mutex<GilrsJoystickState>>) -> Self {
        Self { state }
    }
}

impl JoystickInput for GilrsJoystickInput {
    fn get_axis(&self, joystick: u8, axis: u8) -> f32 {
        let state = self.state.lock().unwrap();
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
        let state = self.state.lock().unwrap();
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
        let state = self.state.lock().unwrap();
        if joystick < 2 {
            state.joysticks[joystick as usize].connected
        } else {
            false
        }
    }
}
