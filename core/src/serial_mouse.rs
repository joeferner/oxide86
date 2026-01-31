//! Serial mouse implementation using Microsoft Serial Mouse protocol.
//!
//! This module implements a serial mouse that communicates via the serial port
//! using the Microsoft Serial Mouse protocol (3-byte packets at 1200 baud, 7N1).
//!
//! # Protocol
//!
//! Microsoft Serial Mouse sends 3-byte packets:
//! - Byte 1: 0x40 | (LB<<5) | (RB<<4) | (Y7<<3) | (Y6<<2) | (X7<<1) | X6
//! - Byte 2: X delta (6-bit signed, -32 to +31)
//! - Byte 3: Y delta (6-bit signed, -32 to +31)
//!
//! Where:
//! - LB = left button (1=pressed)
//! - RB = right button (1=pressed)
//! - X7,X6 = high 2 bits of X delta (sign extension)
//! - Y7,Y6 = high 2 bits of Y delta (sign extension)

use crate::mouse::{MouseInput, MouseState};
use crate::serial_port::{SerialDevice, SerialParams};

/// Serial mouse device implementing Microsoft Serial Mouse protocol
pub struct SerialMouse {
    mouse_input: Box<dyn MouseInput>,
    last_buttons: u8,
    accumulated_x: i16,
    accumulated_y: i16,
    motion_threshold: u16,
    initialized: bool,
}

impl SerialMouse {
    /// Create a new serial mouse wrapping a MouseInput implementation
    pub fn new(mouse_input: Box<dyn MouseInput>) -> Self {
        Self {
            mouse_input,
            last_buttons: 0,
            accumulated_x: 0,
            accumulated_y: 0,
            motion_threshold: 1, // Send packets for every character of movement in text mode
            initialized: false,
        }
    }
}

impl SerialDevice for SerialMouse {
    fn on_init(&mut self, params: &SerialParams) -> Option<Vec<u8>> {
        // Check if initialized to Microsoft Mouse settings (1200 baud, 7N1)
        // 1200 baud = 0x04, 7 bits = 0x02
        if params.baud_rate == 0x04 && params.word_length == 0x02 {
            self.initialized = true;
            // Reset accumulated motion on initialization
            self.accumulated_x = 0;
            self.accumulated_y = 0;
            Some(vec![b'M']) // Send identification byte
        } else {
            None
        }
    }

    fn update(&mut self) -> Vec<u8> {
        // Don't send movement packets until initialized (DTR received)
        if !self.initialized {
            return Vec::new();
        }

        let state = self.mouse_input.get_state();
        let (dx, dy) = self.mouse_input.get_motion();

        self.accumulated_x += dx;
        self.accumulated_y += dy;

        let current_buttons = encode_buttons(&state);
        let buttons_changed = current_buttons != self.last_buttons;
        let motion_exceeded = self.accumulated_x.abs() >= self.motion_threshold as i16
            || self.accumulated_y.abs() >= self.motion_threshold as i16;

        if dx != 0 || dy != 0 {
            log::debug!(
                "SerialMouse: motion dx={}, dy={}, accum_x={}, accum_y={}, threshold={}",
                dx,
                dy,
                self.accumulated_x,
                self.accumulated_y,
                self.motion_threshold
            );
        }

        if buttons_changed || motion_exceeded {
            let packet = generate_ms_mouse_packet(
                state.left_button,
                state.right_button,
                self.accumulated_x,
                self.accumulated_y,
            );

            log::debug!(
                "SerialMouse: Sending packet: {:02X} {:02X} {:02X} (buttons={}/{}, dx={}, dy={})",
                packet[0],
                packet[1],
                packet[2],
                state.left_button,
                state.right_button,
                self.accumulated_x,
                self.accumulated_y
            );

            self.accumulated_x = 0;
            self.accumulated_y = 0;
            self.last_buttons = current_buttons;

            packet.to_vec()
        } else {
            Vec::new()
        }
    }

    fn on_write(&mut self, _byte: u8) {
        // Microsoft Serial Mouse doesn't respond to commands
    }
}

/// Encode button state into a single byte
fn encode_buttons(state: &MouseState) -> u8 {
    let mut buttons = 0u8;
    if state.left_button {
        buttons |= 0x01;
    }
    if state.right_button {
        buttons |= 0x02;
    }
    if state.middle_button {
        buttons |= 0x04;
    }
    buttons
}

/// Generate a Microsoft Serial Mouse protocol packet
///
/// # Arguments
///
/// * `left` - Left button pressed
/// * `right` - Right button pressed
/// * `dx` - X delta (will be clamped to -32..+31)
/// * `dy` - Y delta (will be clamped to -32..+31)
///
/// # Returns
///
/// A 3-byte array containing the packet
fn generate_ms_mouse_packet(left: bool, right: bool, dx: i16, dy: i16) -> [u8; 3] {
    // Clamp deltas to -32..+31 range (6-bit signed)
    let dx = dx.clamp(-32, 31) as i8;
    let dy = dy.clamp(-32, 31) as i8;

    // Extract high bits for byte 1
    let x_hi = ((dx >> 6) & 0x03) as u8;
    let y_hi = ((dy >> 6) & 0x03) as u8;

    // Byte 1: sync bit + buttons + high bits
    let byte1 =
        0x40 | (if left { 0x20 } else { 0 }) | (if right { 0x10 } else { 0 }) | (y_hi << 2) | x_hi;

    // Byte 2: X delta (lower 6 bits)
    let byte2 = (dx & 0x3F) as u8;

    // Byte 3: Y delta (lower 6 bits)
    let byte3 = (dy & 0x3F) as u8;

    [byte1, byte2, byte3]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mouse::NullMouse;

    #[test]
    fn test_encode_buttons() {
        let mut state = MouseState::default();
        assert_eq!(encode_buttons(&state), 0x00);

        state.left_button = true;
        assert_eq!(encode_buttons(&state), 0x01);

        state.right_button = true;
        assert_eq!(encode_buttons(&state), 0x03);

        state.middle_button = true;
        assert_eq!(encode_buttons(&state), 0x07);
    }

    #[test]
    fn test_generate_ms_mouse_packet() {
        // No movement, no buttons
        let packet = generate_ms_mouse_packet(false, false, 0, 0);
        assert_eq!(packet[0], 0x40); // Sync bit only
        assert_eq!(packet[1], 0x00);
        assert_eq!(packet[2], 0x00);

        // Left button pressed
        let packet = generate_ms_mouse_packet(true, false, 0, 0);
        assert_eq!(packet[0], 0x60); // Sync bit + left button (bit 5)

        // Right button pressed
        let packet = generate_ms_mouse_packet(false, true, 0, 0);
        assert_eq!(packet[0], 0x50); // Sync bit + right button (bit 4)

        // Both buttons pressed
        let packet = generate_ms_mouse_packet(true, true, 0, 0);
        assert_eq!(packet[0], 0x70); // Sync bit + both buttons

        // Positive movement
        let packet = generate_ms_mouse_packet(false, false, 10, 5);
        assert_eq!(packet[1], 10); // X delta
        assert_eq!(packet[2], 5); // Y delta

        // Negative movement (6-bit signed: -1 = 0x3F)
        let packet = generate_ms_mouse_packet(false, false, -1, -1);
        assert_eq!(packet[1], 0x3F); // X delta (lower 6 bits)
        assert_eq!(packet[2], 0x3F); // Y delta (lower 6 bits)
    }

    #[test]
    fn test_serial_mouse_on_init() {
        let mouse = Box::new(NullMouse::new());
        let mut serial_mouse = SerialMouse::new(mouse);

        // Microsoft Mouse settings (1200 baud, 7N1)
        let params = SerialParams::from_int14_al(0b10000010);
        let response = serial_mouse.on_init(&params);
        assert_eq!(response, Some(vec![b'M']));

        // Wrong settings
        let params = SerialParams::from_int14_al(0b11100011); // 9600 baud, 8N1
        let response = serial_mouse.on_init(&params);
        assert_eq!(response, None);
    }
}
