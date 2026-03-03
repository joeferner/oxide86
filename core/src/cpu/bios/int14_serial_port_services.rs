use crate::{
    bus::Bus,
    cpu::{Cpu, bios::bda::bda_get_com_port_address},
    devices::uart::{DIVISOR_TABLE, DLL, DLM, LCR, LSR, MCR, MSR, encode_parity},
};

impl Cpu {
    /// INT 0x14 - Serial Port Services
    /// AH register contains the function number
    /// DX register contains the port number (0=COM1, 1=COM2, 2=COM3, 3=COM4)
    pub(in crate::cpu) fn handle_int14_serial_port_services(&mut self, bus: &mut Bus) {
        let function = (self.ax >> 8) as u8; // Get AH
        let port = self.dx as u8; // DX contains port number

        match function {
            0x00 => self.int14_initialize_port(port, bus),
            _ => {
                log::warn!("Unhandled INT 0x14 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 14h, AH=00h - Initialize Serial Port
    /// Input:
    ///   AL = port parameters (baud rate, parity, stop bits, word length)
    ///   DX = port number (0-3 for COM1-COM4)
    /// Output:
    ///   AH = line status
    ///   AL = modem status
    fn int14_initialize_port(&mut self, port: u8, bus: &mut Bus) {
        let init_params = (self.ax & 0xFF) as u8; // Get AL

        // --- 1. Resolve the COM port base I/O address from the BIOS Data Area ---
        let base_addr = bda_get_com_port_address(bus, port);

        // port not present; AH/AL undefined or zeroed
        if base_addr == 0 {
            return;
        }

        // --- 2. Decode init_params byte (AL) ---
        let baud_bits = (init_params >> 5) & 0x07; // bits 7-5
        let parity = (init_params) & 0x03; // bits 4-3
        let stop_bits = (init_params >> 2) & 0x01; // bit 2
        let word_len = init_params & 0x03; // bits 1-0

        // --- 3. Calculate baud rate divisor ---
        let divisor = DIVISOR_TABLE[baud_bits as usize];

        // --- 4. Program the 8250/16550 UART ---

        // 4a. Enable Divisor Latch Access (DLAB=1) so we can write divisor
        let lcr = 0x80; // DLAB bit set
        bus.io_write_u8(base_addr + LCR, lcr);

        // 4b. Write baud rate divisor
        bus.io_write_u8(base_addr + DLL, (divisor & 0xFF) as u8); // low byte
        bus.io_write_u8(base_addr + DLM, ((divisor >> 8) & 0xFF) as u8); // high byte

        // 4c. Build LCR value: word length | stop bits | parity | DLAB=0
        //     word_len: 00->5bit, 01->6bit, 10->7bit, 11->8bit  (maps directly)
        //     stop_bit: 0->1 stop, 1->2 stop (or 1.5 for 5-bit)
        //     parity:   00->none, 01->odd, 11->even (bit3=parity enable, bit4=even)
        let parity_bits = encode_parity(parity); // maps 2-bit field to LCR bits 5-3
        let lcr = word_len | (stop_bits << 2) | parity_bits;
        bus.io_write_u8(base_addr + LCR, lcr); // clears DLAB

        // 4d. Assert DTR and RTS in Modem Control Register
        bus.io_write_u8(base_addr + MCR, 0x03); // DTR=1, RTS=1

        // --- 5. Read back status registers for return values ---
        let ah = bus.io_read_u8(base_addr + LSR); // Line Status Register  -> AH
        let al = bus.io_read_u8(base_addr + MSR); // Modem Status Register -> AL

        // Set return values
        self.ax = ((ah as u16) << 8) | (al as u16);
    }
}
