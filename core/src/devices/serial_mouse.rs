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

use std::collections::VecDeque;

use crate::devices::uart::{ComPortDevice, ModemControlLines};

/// Serial mouse device implementing Microsoft Serial Mouse protocol
pub struct SerialMouse {
    left_button: bool,
    right_button: bool,
    accumulated_x: i16,
    accumulated_y: i16,
    motion_threshold: u16,
    initialized: bool,
    /// Previous DTR state, used to detect rising edge for identification handshake.
    dtr: bool,
    /// Bytes waiting to be read by the UART.
    rx_buf: VecDeque<u8>,
    /// Set when new data is pushed to rx_buf; cleared by take_irq().
    irq_pending: bool,
}

impl SerialMouse {
    pub fn new() -> Self {
        Self {
            left_button: false,
            right_button: false,
            accumulated_x: 0,
            accumulated_y: 0,
            motion_threshold: 1,
            initialized: false,
            dtr: false,
            rx_buf: VecDeque::new(),
            irq_pending: false,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Push accumulated mouse motion to the device. Generates a packet when the
    /// motion threshold is exceeded. No-op until the driver initializes the port
    /// via DTR toggle.
    pub fn push_motion(&mut self, dx: i16, dy: i16) {
        if !self.initialized {
            return;
        }
        self.accumulated_x += dx;
        self.accumulated_y += dy;
        if self.accumulated_x.abs() >= self.motion_threshold as i16
            || self.accumulated_y.abs() >= self.motion_threshold as i16
        {
            self.flush_packet();
        }
    }

    /// Update button state. Generates a packet immediately if the state changed.
    /// No-op until the driver initializes the port via DTR toggle.
    pub fn push_buttons(&mut self, left: bool, right: bool) {
        if !self.initialized {
            return;
        }
        if left != self.left_button || right != self.right_button {
            self.left_button = left;
            self.right_button = right;
            self.flush_packet();
        }
    }

    /// Build and enqueue a 3-byte MS Mouse packet for the current state.
    fn flush_packet(&mut self) {
        let packet = generate_ms_mouse_packet(
            self.left_button,
            self.right_button,
            self.accumulated_x,
            self.accumulated_y,
        );
        self.accumulated_x = 0;
        self.accumulated_y = 0;
        log::debug!(
            "SerialMouse: queued packet {:02X} {:02X} {:02X}",
            packet[0],
            packet[1],
            packet[2]
        );
        for byte in packet {
            self.rx_buf.push_back(byte);
        }
        self.irq_pending = true;
    }
}

impl ComPortDevice for SerialMouse {
    fn reset(&mut self) {
        self.initialized = false;
        self.dtr = false;
        self.accumulated_x = 0;
        self.accumulated_y = 0;
        self.left_button = false;
        self.right_button = false;
        self.rx_buf.clear();
        self.irq_pending = false;
    }

    fn read(&mut self) -> Option<u8> {
        self.rx_buf.pop_front()
    }

    fn write(&mut self, _value: u8) -> bool {
        // Microsoft Serial Mouse doesn't respond to commands
        false
    }

    fn take_irq(&mut self) -> bool {
        let pending = self.irq_pending;
        self.irq_pending = false;
        pending
    }

    fn modem_control_changed(&mut self, lines: ModemControlLines) {
        let prev_dtr = self.dtr;
        self.dtr = lines.dtr;

        if lines.dtr && !prev_dtr {
            // DTR rising edge: the driver is initializing the mouse.
            // Send the 'M' identification byte and start accepting input.
            log::debug!("SerialMouse: DTR raised, sending 'M' identification");
            self.initialized = true;
            self.accumulated_x = 0;
            self.accumulated_y = 0;
            self.left_button = false;
            self.right_button = false;
            self.rx_buf.clear();
            self.rx_buf.push_back(b'M');
            self.irq_pending = true;
        } else if !lines.dtr && prev_dtr {
            // DTR dropped: deactivate until next rising edge.
            log::debug!("SerialMouse: DTR dropped, deactivating");
            self.initialized = false;
            self.rx_buf.clear();
            self.irq_pending = false;
        }
    }
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
}
