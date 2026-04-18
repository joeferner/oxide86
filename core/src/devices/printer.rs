//! Stub printer device for LPT1.
//!
//! Implements [`LptPortDevice`] and logs every data byte, control change, and
//! strobe-triggered write. No actual output is produced; this is purely for
//! debugging what a guest program sends to the parallel port.

use crate::{byte_to_printable_char, devices::parallel_port::LptPortDevice};

/// Stub printer that logs all LPT activity.
pub struct Printer {
    /// Last data byte placed on the parallel bus.
    data: u8,
}

impl Printer {
    pub fn new() -> Self {
        Self { data: 0 }
    }
}

impl LptPortDevice for Printer {
    fn reset(&mut self) {
        log::debug!("[printer] reset");
        self.data = 0;
    }

    fn data_changed(&mut self, data: u8) {
        log::debug!(
            "[printer] data register <- 0x{:02X} ('{}')",
            data,
            byte_to_printable_char(data)
        );
        self.data = data;
    }

    fn control_changed(&mut self, control: u8) {
        log::debug!("[printer] control register <- 0x{:02X}", control);
    }

    fn write(&mut self, data: u8) -> bool {
        log::debug!(
            "[printer] strobe write 0x{:02X} ('{}')",
            data,
            byte_to_printable_char(data)
        );
        true
    }

    fn status(&mut self) -> u8 {
        // Report: not busy, ACK, selected, no error, no paper-out.
        // Matches STATUS_READY in parallel_port.rs (0xDF).
        0xDF
    }
}
