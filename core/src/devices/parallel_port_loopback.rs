//! Parallel port loopback device.
//!
//! Simulates the standard IBM PC LPT loopback plug used by diagnostic
//! programs such as CheckIt. The plug wires the control output pins back to
//! the status input pins, allowing software to verify that the parallel port
//! I/O hardware works correctly without an attached printer.
//!
//! ## Loopback wiring
//!
//! | Control pin | → | Status pin | Control bit | Status bit |
//! |-------------|---|------------|-------------|------------|
//! | Pin 1  (Strobe, inverted)   | → | Pin 11 (Busy, inv) | bit 0 | bit 7 |
//! | Pin 14 (Auto-LF, inverted)  | → | Pin 12 (Paper Out) | bit 1 | bit 5 |
//! | Pin 16 (/Init, not inverted)| → | Pin 10 (/ACK)      | bit 2 | bit 6 |
//! | Pin 17 (Sel-In, inverted)   | → | Pin 13 (Select)    | bit 3 | bit 4 |
//! | Pin 2  (Data D0)            | → | Pin 15 (/Error)    | —     | bit 3 |
//!
//! ## Status formula (from register bits, not pin levels)
//!
//! The control register inverts bits 0, 1, and 3 at the hardware pin level
//! (bit=1 drives the pin LOW). Bit 2 is not inverted (bit=0 drives pin LOW).
//! The status register inverts pin 11 at the hardware level.
//!
//! Working through the inversions:
//! - status bit 7 = control bit 0        (strobe → NOT-BUSY, double-inversion cancels)
//! - status bit 6 = control bit 2        (/Init → /ACK, no inversion either side)
//! - status bit 5 = NOT(control bit 1)   (Auto-LF inverted at pin, Paper Out not inverted)
//! - status bit 4 = NOT(control bit 3)   (Sel-In inverted at pin, Select not inverted)
//! - status bit 3 = data bit 0           (D0 drives /ERROR pin directly)

use crate::devices::parallel_port::LptPortDevice;

/// LPT loopback plug: wires control outputs back to status inputs.
pub struct ParallelLoopback {
    data: u8,
    control: u8,
}

impl ParallelLoopback {
    pub fn new() -> Self {
        Self {
            data: 0,
            control: 0,
        }
    }
}

impl LptPortDevice for ParallelLoopback {
    fn reset(&mut self) {
        self.data = 0;
        self.control = 0;
    }

    fn data_changed(&mut self, data: u8) {
        self.data = data;
    }

    fn control_changed(&mut self, control: u8) {
        self.control = control;
    }

    fn write(&mut self, data: u8) -> bool {
        self.data = data;
        true
    }

    fn status(&mut self) -> u8 {
        // See module-level doc for the derivation of each bit.
        let bit7 = (self.control & 0x01) << 7; // strobe → NOT-BUSY
        let bit6 = ((self.control >> 2) & 0x01) << 6; // /Init → /ACK
        let bit5 = ((!self.control >> 1) & 0x01) << 5; // NOT Auto-LF → Paper Out
        let bit4 = ((!self.control >> 3) & 0x01) << 4; // NOT Sel-In → Select
        let bit3 = (self.data & 0x01) << 3; // D0 → /Error
        bit7 | bit6 | bit5 | bit4 | bit3 | 0x07 // bits 2:0 reserved, set to 1
    }
}
