use crate::{cpu::Cpu, io_bus::IoBus, memory_bus::MemoryBus};

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

impl PrinterStatus {
    /// Create a ready status (printer ready, no errors)
    pub fn ready() -> Self {
        Self {
            status: printer_status::NOT_BUSY
                | printer_status::SELECTED
                | printer_status::ACKNOWLEDGE,
        }
    }

    /// Create a timeout status
    pub fn timeout() -> Self {
        Self {
            status: printer_status::TIMEOUT,
        }
    }
}

impl Cpu {
    /// INT 0x17 - Printer Services
    /// AH register contains the function number
    /// DX register contains the printer number (0=LPT1, 1=LPT2, 2=LPT3)
    pub(in crate::cpu) fn handle_int17_printer_services(
        &mut self,
        _memory_bus: &mut MemoryBus,
        io_bus: &mut IoBus,
    ) {
        let function = (self.ax >> 8) as u8; // Get AH
        let printer = self.dx as u8; // DX contains printer number

        match function {
            0x01 => self.int17_initialize_printer(printer, io_bus),
            _ => {
                log::warn!("Unhandled INT 0x17 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 17h, AH=01h - Initialize Printer Port
    /// Input:
    ///   DX = printer number
    /// Output:
    ///   AH = printer status
    fn int17_initialize_printer(&mut self, printer: u8, io_bus: &mut IoBus) {
        let status = io_bus.printer_init(printer);

        // Set AH to printer status
        self.ax = (self.ax & 0x00FF) | ((status.status as u16) << 8);
    }
}
