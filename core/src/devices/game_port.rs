//! PC Game Port (joystick) device implementation.
//!
//! Emulates the IBM PC Game Control Adapter at I/O port 0x201.
//!
//! # Protocol
//!
//! Writing any value to 0x201 fires the one-shot timing circuit for all axes.
//! Reading 0x201 returns an 8-bit status byte:
//!
//! ```text
//! Bit 7: Button 4 (0 = pressed)
//! Bit 6: Button 3 (0 = pressed)
//! Bit 5: Button 2 (0 = pressed)
//! Bit 4: Button 1 (0 = pressed)
//! Bit 3: Joystick 2 Y-axis one-shot (1 = timing, 0 = done)
//! Bit 2: Joystick 2 X-axis one-shot (1 = timing, 0 = done)
//! Bit 1: Joystick 1 Y-axis one-shot (1 = timing, 0 = done)
//! Bit 0: Joystick 1 X-axis one-shot (1 = timing, 0 = done)
//! ```
//!
//! Each axis one-shot bit stays high for approximately `axis_value * 11µs`.
//! In CPU cycles: `axis_value * clock_speed / 90_909`.
//! Axis values range 0–255, with 128 representing center.

use std::any::Any;

use crate::Device;

pub const GAME_PORT: u16 = 0x201;

/// PC Game Port device implementing the IBM Game Control Adapter protocol.
pub struct GamePortDevice {
    /// Axis positions: [x1, y1, x2, y2], 0..=255, center = 128.
    axes: [u8; 4],
    /// Button state bits in positions 4-7; 0 = pressed (inverted).
    /// Bits 0-3 are unused (always 0).
    buttons: u8,
    /// CPU cycle count when 0x201 was last written (one-shot reset).
    reset_cycle: u32,
    /// Whether the one-shot circuit has been triggered by a write to 0x201.
    /// Before the first write, all timing bits read as 0 (timed out).
    one_shot_fired: bool,
    /// CPU clock speed in Hz, used to compute one-shot timing.
    clock_speed: u32,
}

impl GamePortDevice {
    pub fn new(clock_speed: u32) -> Self {
        Self {
            axes: [128, 128, 128, 128],
            buttons: 0xF0, // all buttons released
            reset_cycle: 0,
            one_shot_fired: false,
            clock_speed,
        }
    }

    /// Set axis positions. Values are 0..=255 with 128 as center.
    pub fn set_axes(&mut self, x1: u8, y1: u8, x2: u8, y2: u8) {
        self.axes = [x1, y1, x2, y2];
    }

    /// Set button states. `true` = pressed.
    pub fn set_buttons(&mut self, b1: bool, b2: bool, b3: bool, b4: bool) {
        // Buttons are inverted: 0 = pressed, 1 = released; stored in bits 4-7.
        let b = |pressed: bool, bit: u8| if pressed { 0u8 } else { bit };
        self.buttons = b(b1, 0x10) | b(b2, 0x20) | b(b3, 0x40) | b(b4, 0x80);
    }
}

impl Device for GamePortDevice {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.axes = [128, 128, 128, 128];
        self.buttons = 0xF0;
        self.reset_cycle = 0;
        self.one_shot_fired = false;
    }

    fn memory_read_u8(&self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16, cycle_count: u32) -> Option<u8> {
        if port != GAME_PORT {
            return None;
        }
        // Before the first write, all timing bits read as 0 (one-shot not triggered).
        if !self.one_shot_fired {
            return Some(self.buttons);
        }
        let elapsed = cycle_count.saturating_sub(self.reset_cycle);
        let mut timing_bits = 0u8;
        for (i, &axis) in self.axes.iter().enumerate() {
            // cycles_needed = axis * clock_speed / 90_909  (≈ axis * 11µs)
            let cycles_needed = (axis as u32).saturating_mul(self.clock_speed) / 90_909;
            if elapsed < cycles_needed {
                timing_bits |= 1 << i;
            }
        }
        Some(self.buttons | timing_bits)
    }

    fn io_write_u8(&mut self, port: u16, _val: u8, cycle_count: u32) -> bool {
        if port != GAME_PORT {
            return false;
        }
        // Any write fires the one-shot timing circuit.
        self.reset_cycle = cycle_count;
        self.one_shot_fired = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLOCK: u32 = 4_772_727; // ~4.77 MHz (original IBM PC)

    fn make() -> GamePortDevice {
        GamePortDevice::new(CLOCK)
    }

    #[test]
    fn test_buttons_released_by_default() {
        let js = make();
        // No reset yet, timing bits all 0; buttons all released = bits 4-7 high
        let status = js.io_read_u8(GAME_PORT, 0).unwrap();
        assert_eq!(status & 0xF0, 0xF0, "all buttons should be released");
        assert_eq!(status & 0x0F, 0x00, "no timing (no reset fired)");
    }

    #[test]
    fn test_button_pressed() {
        let mut js = make();
        js.set_buttons(true, false, false, false); // button 1 pressed
        let status = js.io_read_u8(GAME_PORT, 0).unwrap();
        assert_eq!(status & 0x10, 0x00, "button 1 bit should be 0 (pressed)");
        assert_eq!(status & 0xE0, 0xE0, "other button bits should remain high");
    }

    #[test]
    fn test_axis_timing_starts_after_write() {
        let mut js = make();
        js.set_axes(128, 128, 128, 128); // center
        // Fire one-shot at cycle 0
        js.io_write_u8(GAME_PORT, 0, 0);
        // Immediately after: all timing bits should be high (axis > 0)
        let status = js.io_read_u8(GAME_PORT, 0).unwrap();
        assert_eq!(
            status & 0x0F,
            0x0F,
            "all timing bits high right after write"
        );
    }

    #[test]
    fn test_axis_timing_clears_after_enough_cycles() {
        let mut js = make();
        js.set_axes(128, 128, 128, 128);
        js.io_write_u8(GAME_PORT, 0, 0);
        // cycles needed for axis=128 at 4.77MHz ≈ 128 * 4_772_727 / 90_909 ≈ 6724
        let cycles_needed = 128u32 * CLOCK / 90_909 + 1;
        let status = js.io_read_u8(GAME_PORT, cycles_needed).unwrap();
        assert_eq!(status & 0x0F, 0x00, "all timing bits should be cleared");
    }

    #[test]
    fn test_axis_zero_never_times() {
        let mut js = make();
        js.set_axes(0, 0, 0, 0); // minimum resistance (joystick hard against edge)
        js.io_write_u8(GAME_PORT, 0, 0);
        // cycles_needed = 0 * CLOCK / 90_909 = 0, so elapsed(0) < 0 is false → bit = 0
        let status = js.io_read_u8(GAME_PORT, 0).unwrap();
        assert_eq!(status & 0x0F, 0x00, "zero axis should not time");
    }

    #[test]
    fn test_ignores_other_ports() {
        let mut js = make();
        assert!(js.io_read_u8(0x200, 0).is_none());
        assert!(!js.io_write_u8(0x200, 0, 0));
    }
}
