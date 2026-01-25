use emu86_core::cpu::bios::int14::line_status;
use emu86_core::cpu::bios::int17::printer_status;
use emu86_core::cpu::bios::{PrinterStatus, SerialParams, SerialStatus};

// Serial port and printer operations for NativeBios
// These are stub implementations as actual hardware is not available

pub fn serial_init(_port: u8, _params: SerialParams) -> SerialStatus {
    // Serial port not available in stdio implementation
    SerialStatus {
        line_status: line_status::TIMEOUT,
        modem_status: 0,
    }
}

pub fn serial_write(_port: u8, _ch: u8) -> u8 {
    // Serial port not available - return timeout
    line_status::TIMEOUT
}

pub fn serial_read(_port: u8) -> Result<(u8, u8), u8> {
    // Serial port not available - return timeout error
    Err(line_status::TIMEOUT)
}

pub fn serial_status(_port: u8) -> SerialStatus {
    // Serial port not available
    SerialStatus {
        line_status: line_status::TIMEOUT,
        modem_status: 0,
    }
}

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
