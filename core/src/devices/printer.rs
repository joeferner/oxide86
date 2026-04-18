//! Printer device for LPT ports.
//!
//! Streams the raw byte sequence sent over the parallel port directly into a
//! [`Write`] sink provided at construction time. The caller decides where the
//! bytes go — a file on native, a shared in-memory buffer on WASM, or
//! [`std::io::sink`] to discard output.
//!
//! The raw bytes are an unmodified copy of whatever the guest program sent to
//! the printer port — ESC/P commands, plain text, PCL, PostScript, or anything
//! else. A separate conversion tool can interpret them later.

use std::io::Write;

use crate::devices::parallel_port::LptPortDevice;

/// Printer that forwards each byte received via strobe to an inner [`Write`] sink.
pub struct Printer {
    writer: Box<dyn Write + Send + Sync>,
}

impl Printer {
    pub fn new(writer: Box<dyn Write + Send + Sync>) -> Self {
        Self { writer }
    }
}

impl LptPortDevice for Printer {
    fn reset(&mut self) {
        log::debug!("[printer] reset");
        let _ = self.writer.flush();
    }

    fn write(&mut self, data: u8) -> bool {
        if let Err(e) = self.writer.write_all(&[data]) {
            log::warn!("[printer] write error: {e}");
        }
        true
    }

    fn status(&mut self) -> u8 {
        // Report: not busy, ACK, selected, no error, no paper-out (0xDF).
        0xDF
    }
}
