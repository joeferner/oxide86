use crate::cpu::bios::PrinterStatus;
use crate::cpu::bios::int17::printer_status;

// printer operations stub implementations
// These return timeout status as actual hardware is not available
// Used by both native CLI and GUI frontends

pub fn printer_init(_printer: u8) -> PrinterStatus {
    // No printer available - return timeout status
    PrinterStatus {
        status: printer_status::TIMEOUT,
    }
}

pub fn printer_write(_printer: u8, _ch: u8) -> PrinterStatus {
    // No printer available - return timeout status
    PrinterStatus {
        status: printer_status::TIMEOUT,
    }
}

pub fn printer_status(_printer: u8) -> PrinterStatus {
    // No printer available - return timeout status
    PrinterStatus {
        status: printer_status::TIMEOUT,
    }
}
