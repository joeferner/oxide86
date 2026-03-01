use crate::cpu::Cpu;


impl Cpu {
    pub(super) fn handle_int17(&mut self, io: &mut super::Bios) {
        match function {
            0x00 => self.int17_print_char(printer, io),
            0x02 => self.int17_get_status(printer, io),
        }
    }

    /// INT 17h, AH=00h - Print Character
    /// Input:
    ///   AL = character to print
    ///   DX = printer number (0-2 for LPT1-LPT3)
    /// Output:
    ///   AH = printer status
    fn int17_print_char(&mut self, printer: u8, io: &mut super::Bios) {
        let ch = (self.ax & 0xFF) as u8; // Get AL

        let status = io.printer_write(printer, ch);

        // Set AH to printer status, keep AL unchanged
        self.ax = (self.ax & 0x00FF) | ((status.status as u16) << 8);
    }

    /// INT 17h, AH=02h - Get Printer Status
    /// Input:
    ///   DX = printer number
    /// Output:
    ///   AH = printer status
    fn int17_get_status(&mut self, printer: u8, io: &mut super::Bios) {
        let status = io.printer_status(printer);

        // Set AH to printer status
        self.ax = (self.ax & 0x00FF) | ((status.status as u16) << 8);
    }
}
