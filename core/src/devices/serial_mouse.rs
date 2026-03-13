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
//!
//! # Packet generation
//!
//! Packets are generated lazily: `push_motion` and `push_buttons` only accumulate
//! state and assert `irq_pending`. The actual 3-byte packet is built the first time
//! `read` is called with an empty receive buffer. This ensures that all motion
//! accumulated between IRQ firings is captured in a single packet rather than many
//! small ones, which is more faithful to a real serial mouse sending at 1200 baud.
//!
//! After each byte is returned from `read`, `irq_pending` is re-armed while bytes
//! remain in the buffer, so the driver's interrupt handler can drain a full packet
//! without stalling.

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
    /// Asserted by IRQ mechanism; cleared by take_irq() and re-armed by read() while
    /// bytes remain in rx_buf.
    irq_pending: bool,
    /// True when motion or button state has changed and a packet has not yet been sent.
    /// Set by push_motion/push_buttons; cleared when a packet is generated in read().
    data_pending: bool,
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
            data_pending: false,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Push accumulated mouse motion to the device. Signals an IRQ when the motion
    /// threshold is exceeded. The packet is generated lazily on the next `read` call.
    /// No-op until the driver initializes the port via DTR toggle.
    pub fn push_motion(&mut self, dx: i16, dy: i16) {
        if !self.initialized {
            return;
        }
        log::debug!("push motion dx:{dx}, dy:{dy}");
        self.accumulated_x += dx;
        self.accumulated_y += dy;
        if self.accumulated_x.abs() >= self.motion_threshold as i16
            || self.accumulated_y.abs() >= self.motion_threshold as i16
        {
            self.irq_pending = true;
            self.data_pending = true;
        }
    }

    /// Update button state. Signals an IRQ if the state changed. The packet is
    /// generated lazily on the next `read` call. No-op until the driver initializes
    /// the port via DTR toggle.
    pub fn push_buttons(&mut self, left: bool, right: bool) {
        if !self.initialized {
            return;
        }
        if left != self.left_button || right != self.right_button {
            log::debug!("button changed left:{left}, right:{right}");
            self.left_button = left;
            self.right_button = right;
            self.irq_pending = true;
            self.data_pending = true;
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
        self.data_pending = false;
        log::debug!(
            "queued packet {:02X} {:02X} {:02X}",
            packet[0],
            packet[1],
            packet[2]
        );
        for byte in packet {
            self.rx_buf.push_back(byte);
        }
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
        self.data_pending = false;
    }

    /// Return the next byte from the receive buffer.
    ///
    /// If the buffer is empty and a packet is pending, the packet is generated now
    /// (lazy generation). After returning a byte, `irq_pending` is re-armed while
    /// bytes remain, so the interrupt handler can drain a complete 3-byte packet.
    fn read(&mut self) -> Option<u8> {
        if self.rx_buf.is_empty() && self.data_pending {
            self.flush_packet();
        }
        let byte = self.rx_buf.pop_front();
        // Re-arm the IRQ so the driver keeps reading until the buffer is drained.
        self.irq_pending = !self.rx_buf.is_empty();
        byte
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
            self.data_pending = false;
            self.rx_buf.clear();
            self.rx_buf.push_back(b'M');
            self.irq_pending = true;
        } else if !lines.dtr && prev_dtr {
            // DTR dropped: deactivate until next rising edge.
            log::debug!("SerialMouse: DTR dropped, deactivating");
            self.initialized = false;
            self.rx_buf.clear();
            self.irq_pending = false;
            self.data_pending = false;
        }
    }
}

/// Generate a Microsoft Serial Mouse protocol packet
///
/// # Arguments
///
/// * `left` - Left button pressed
/// * `right` - Right button pressed
/// * `dx` - X delta (will be clamped to -128..+127)
/// * `dy` - Y delta (will be clamped to -128..+127)
///
/// # Returns
///
/// A 3-byte array containing the packet
fn generate_ms_mouse_packet(left: bool, right: bool, dx: i16, dy: i16) -> [u8; 3] {
    // Clamp deltas to -128..+127 range (8-bit signed: 2 high bits in byte 0 + 6 low bits in bytes 1-2)
    let dx = dx.clamp(-128, 127) as i8;
    let dy = dy.clamp(-128, 127) as i8;

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

        // Negative movement (-1 = 0xFF: high bits 0x03 in byte 0, low bits 0x3F in bytes 1-2)
        let packet = generate_ms_mouse_packet(false, false, -1, -1);
        assert_eq!(packet[0] & 0x03, 0x03); // X high bits = 0b11 (sign)
        assert_eq!(packet[0] & 0x0C, 0x0C); // Y high bits = 0b11 (sign)
        assert_eq!(packet[1], 0x3F); // X delta lower 6 bits
        assert_eq!(packet[2], 0x3F); // Y delta lower 6 bits

        // Large positive movement (127 = 0x7F: high bits 0x01, low bits 0x3F)
        let packet = generate_ms_mouse_packet(false, false, 127, 0);
        assert_eq!(packet[0] & 0x03, 0x01); // X high bits = 0b01
        assert_eq!(packet[1], 0x3F); // X lower 6 bits
        assert_eq!(packet[2], 0x00);

        // Large negative movement (-128 = 0x80: high bits 0x02, low bits 0x00)
        let packet = generate_ms_mouse_packet(false, false, -128, 0);
        assert_eq!(packet[0] & 0x03, 0x02); // X high bits = 0b10
        assert_eq!(packet[1], 0x00); // X lower 6 bits
    }

    #[test]
    fn test_lazy_packet_generation() {
        let mut mouse = SerialMouse::new();
        let lines = crate::devices::uart::ModemControlLines {
            dtr: true,
            rts: false,
            out1: false,
            out2: false,
            loopback: false,
        };
        mouse.modem_control_changed(lines);
        // Drain 'M' identification byte
        assert_eq!(mouse.read(), Some(b'M'));

        // Accumulate motion across multiple push_motion calls
        mouse.push_motion(10, 0);
        mouse.push_motion(15, 0);
        mouse.push_motion(5, 3);
        assert!(mouse.take_irq()); // IRQ should be pending

        // Packet is generated lazily on first read
        let b1 = mouse.read().expect("byte 1");
        assert_eq!(b1 & 0x40, 0x40); // sync bit
        assert_eq!(b1 & 0x03, 0x00); // x_hi = 0 (30 fits in 6 bits)
        assert!(mouse.irq_pending); // re-armed for bytes 2 and 3

        let b2 = mouse.read().expect("byte 2");
        assert_eq!(b2, 30); // accumulated x = 10+15+5 = 30
        assert!(mouse.irq_pending);

        let b3 = mouse.read().expect("byte 3");
        assert_eq!(b3, 3); // accumulated y = 3
        assert!(!mouse.irq_pending); // buffer drained
    }
}
