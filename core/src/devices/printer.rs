//! Stub printer device for LPT1.
//!
//! Accumulates the raw byte stream sent over the parallel port into an internal
//! buffer. Callers drain it with [`Printer::take_output`] and are responsible
//! for writing the bytes to a file or forwarding them elsewhere.
//!
//! The raw bytes are an unmodified copy of whatever the guest program sent to
//! the printer port — ESC/P commands, plain text, PCL, PostScript, or anything
//! else. A separate conversion tool can interpret the bytes later.

use crate::devices::parallel_port::LptPortDevice;

/// Stub printer that buffers all LPT output for later retrieval.
pub struct Printer {
    output: Vec<u8>,
}

impl Printer {
    pub fn new() -> Self {
        Self { output: Vec::new() }
    }
}

impl LptPortDevice for Printer {
    fn reset(&mut self) {
        log::debug!("[printer] reset");
        self.output.clear();
    }

    fn write(&mut self, data: u8) -> bool {
        self.output.push(data);
        true
    }

    fn status(&mut self) -> u8 {
        // Report: not busy, ACK, selected, no error, no paper-out.
        // Matches STATUS_READY in parallel_port.rs (0xDF).
        0xDF
    }

    fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output)
    }
}
