use crate::bus::Bus;

/// Printer status bits (returned in AH)
pub mod printer_status {
    pub const TIMEOUT: u8 = 0x01;
    pub const IO_ERROR: u8 = 0x08;
    pub const SELECTED: u8 = 0x10;
    pub const OUT_OF_PAPER: u8 = 0x20;
    pub const ACKNOWLEDGE: u8 = 0x40;
    pub const NOT_BUSY: u8 = 0x80; // 0 = busy, 1 = ready
}

/// Printer status returned by operations
#[derive(Debug, Clone, Copy)]
pub struct PrinterStatus {
    /// Status byte (returned in AH)
    pub status: u8,
}

pub(crate) fn printer_init(_bus: &Bus, _printer: u8) -> PrinterStatus {
    // No printer available - return timeout status
    PrinterStatus {
        status: printer_status::TIMEOUT,
    }
}
