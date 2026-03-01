use crate::{bus::Bus, cpu::Cpu, devices::printer::printer_init};

impl Cpu {
    /// INT 0x17 - Printer Services
    /// AH register contains the function number
    /// DX register contains the printer number (0=LPT1, 1=LPT2, 2=LPT3)
    pub(in crate::cpu) fn handle_int17_printer_services(&mut self, bus: &mut Bus) {
        let function = (self.ax >> 8) as u8; // Get AH
        let printer = self.dx as u8; // DX contains printer number

        match function {
            0x01 => self.int17_initialize_printer(printer, bus),
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
    fn int17_initialize_printer(&mut self, printer: u8, bus: &mut Bus) {
        let status = printer_init(bus, printer);

        // Set AH to printer status
        self.ax = (self.ax & 0x00FF) | ((status.status as u16) << 8);
    }
}
