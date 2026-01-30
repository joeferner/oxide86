use crate::{cpu::Cpu, memory::Memory};

/// Printer status bits (returned in AH)
#[allow(dead_code)]
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
    pub(super) fn handle_int17<K: crate::KeyboardInput, D: crate::DiskController>(
        &mut self,
        _memory: &mut Memory,
        io: &mut super::Bios<K, D>,
    ) {
        let function = (self.ax >> 8) as u8; // Get AH
        let printer = self.dx as u8; // DX contains printer number

        match function {
            0x00 => self.int17_print_char(printer, io),
            0x01 => self.int17_initialize_printer(printer, io),
            0x02 => self.int17_get_status(printer, io),
            _ => {
                log::warn!("Unhandled INT 0x17 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 17h, AH=00h - Print Character
    /// Input:
    ///   AL = character to print
    ///   DX = printer number (0-2 for LPT1-LPT3)
    /// Output:
    ///   AH = printer status
    fn int17_print_char<K: crate::KeyboardInput, D: crate::DiskController>(
        &mut self,
        printer: u8,
        io: &mut super::Bios<K, D>,
    ) {
        let ch = (self.ax & 0xFF) as u8; // Get AL

        let status = io.printer_write(printer, ch);

        // Set AH to printer status, keep AL unchanged
        self.ax = (self.ax & 0x00FF) | ((status.status as u16) << 8);
    }

    /// INT 17h, AH=01h - Initialize Printer Port
    /// Input:
    ///   DX = printer number
    /// Output:
    ///   AH = printer status
    fn int17_initialize_printer<K: crate::KeyboardInput, D: crate::DiskController>(
        &mut self,
        printer: u8,
        io: &mut super::Bios<K, D>,
    ) {
        let status = io.printer_init(printer);

        // Set AH to printer status
        self.ax = (self.ax & 0x00FF) | ((status.status as u16) << 8);
    }

    /// INT 17h, AH=02h - Get Printer Status
    /// Input:
    ///   DX = printer number
    /// Output:
    ///   AH = printer status
    fn int17_get_status<K: crate::KeyboardInput, D: crate::DiskController>(
        &mut self,
        printer: u8,
        io: &mut super::Bios<K, D>,
    ) {
        let status = io.printer_status(printer);

        // Set AH to printer status
        self.ax = (self.ax & 0x00FF) | ((status.status as u16) << 8);
    }
}
