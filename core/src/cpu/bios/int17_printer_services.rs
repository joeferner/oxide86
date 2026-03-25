use crate::{bus::Bus, cpu::Cpu};

/// BDA physical address of LPT port table (4 words at 0040:0008).
const BDA_LPT_PORTS: usize = 0x0408;

impl Cpu {
    /// INT 0x17 - Printer Services
    /// AH register contains the function number
    /// DX register contains the printer number (0=LPT1, 1=LPT2, 2=LPT3)
    pub(in crate::cpu) fn handle_int17_printer_services(&mut self, bus: &mut Bus) {
        bus.increment_cycle_count(200);
        let function = (self.ax >> 8) as u8;
        let printer = (self.dx & 0x03) as usize;

        // Look up port base from BDA
        let base = bus.memory_read_u16(BDA_LPT_PORTS + printer * 2);
        if base == 0 {
            // No port installed — return timeout
            self.ax = (self.ax & 0x00FF) | (0x01 << 8); // AH = timeout bit
            return;
        }

        match function {
            0x00 => self.int17_print_character(bus, base),
            0x01 => self.int17_initialize_printer(bus, base),
            0x02 => self.int17_get_status(bus, base),
            _ => {
                log::warn!("Unhandled INT 0x17 function: AH=0x{:02X}", function);
            }
        }
    }

    /// AH=00h - Print Character
    /// Writes AL to the data port, pulses strobe, returns status in AH.
    fn int17_print_character(&mut self, bus: &mut Bus, base: u16) {
        let ch = (self.ax & 0xFF) as u8;

        // Write character to data register
        bus.io_write_u8(base, ch);

        // Pulse strobe: set bit 0, then clear it
        let ctrl = bus.io_read_u8(base + 2);
        bus.io_write_u8(base + 2, ctrl | 0x01);
        bus.io_write_u8(base + 2, ctrl & !0x01);

        // Return status in AH
        let status = lpt_status_to_bios(bus.io_read_u8(base + 1));
        self.ax = (self.ax & 0x00FF) | ((status as u16) << 8);
    }

    /// AH=01h - Initialize Printer Port
    /// Pulses the INIT line on the control register, returns status in AH.
    fn int17_initialize_printer(&mut self, bus: &mut Bus, base: u16) {
        // Assert INIT (bit 2 = 0), then de-assert (bit 2 = 1)
        bus.io_write_u8(base + 2, 0x08); // select printer, INIT low
        // Small delay would happen on real HW; we just toggle immediately
        bus.io_write_u8(base + 2, 0x0C); // select printer, INIT high

        let status = lpt_status_to_bios(bus.io_read_u8(base + 1));
        self.ax = (self.ax & 0x00FF) | ((status as u16) << 8);
    }

    /// AH=02h - Get Printer Status
    /// Reads the status register and returns it in AH.
    fn int17_get_status(&mut self, bus: &mut Bus, base: u16) {
        let status = lpt_status_to_bios(bus.io_read_u8(base + 1));
        self.ax = (self.ax & 0x00FF) | ((status as u16) << 8);
    }
}

/// Convert the hardware status register to the BIOS status byte.
/// The hardware status register bits 3–7 map directly to the BIOS
/// status byte, but bit 6 (/ACK) is inverted in the BIOS convention.
fn lpt_status_to_bios(hw_status: u8) -> u8 {
    (hw_status ^ 0x48) & 0xF8
}
