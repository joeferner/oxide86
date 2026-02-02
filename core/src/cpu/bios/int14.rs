use crate::{cpu::Cpu, memory::Memory, serial_port::SerialParams};

/// Serial port line status bits (returned in AH)
#[allow(dead_code)]
pub mod line_status {
    pub const TIMEOUT: u8 = 0x80;
    pub const TRANSMIT_SHIFT_EMPTY: u8 = 0x40;
    pub const TRANSMIT_HOLDING_EMPTY: u8 = 0x20;
    pub const BREAK_DETECT: u8 = 0x10;
    pub const FRAMING_ERROR: u8 = 0x08;
    pub const PARITY_ERROR: u8 = 0x04;
    pub const OVERRUN_ERROR: u8 = 0x02;
    pub const DATA_READY: u8 = 0x01;
}

/// Serial port modem status bits (returned in AL)
#[allow(dead_code)]
pub mod modem_status {
    pub const RECEIVED_LINE_SIGNAL_DETECT: u8 = 0x80;
    pub const RING_INDICATOR: u8 = 0x40;
    pub const DATA_SET_READY: u8 = 0x20;
    pub const CLEAR_TO_SEND: u8 = 0x10;
    pub const CHANGE_RECEIVE_LINE_SIGNAL: u8 = 0x08;
    pub const TRAILING_EDGE_RING: u8 = 0x04;
    pub const CHANGE_DATA_SET_READY: u8 = 0x02;
    pub const CHANGE_CLEAR_TO_SEND: u8 = 0x01;
}

impl Cpu {
    /// INT 0x14 - Serial Port Services
    /// AH register contains the function number
    /// DX register contains the port number (0=COM1, 1=COM2, 2=COM3, 3=COM4)
    pub(super) fn handle_int14(&mut self, _memory: &mut Memory, io: &mut super::Bios) {
        let function = (self.ax >> 8) as u8; // Get AH
        let port = self.dx as u8; // DX contains port number

        match function {
            0x00 => self.int14_initialize_port(port, io),
            0x01 => self.int14_write_char(port, io),
            0x02 => self.int14_read_char(port, io),
            0x03 => self.int14_get_status(port, io),
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
    fn int14_initialize_port(&mut self, port: u8, io: &mut super::Bios) {
        let params_byte = (self.ax & 0xFF) as u8; // Get AL
        let params = SerialParams::from_int14_al(params_byte);

        let status = io.serial_init(port, params);

        // Set return values
        self.ax = ((status.line_status as u16) << 8) | (status.modem_status as u16);
    }

    /// INT 14h, AH=01h - Write Character to Serial Port
    /// Input:
    ///   AL = character to transmit
    ///   DX = port number
    /// Output:
    ///   AH = line status (bit 7 set if timeout)
    fn int14_write_char(&mut self, port: u8, io: &mut super::Bios) {
        let ch = (self.ax & 0xFF) as u8; // Get AL

        let status = io.serial_write(port, ch);

        // Set AH to line status, keep AL unchanged
        self.ax = (self.ax & 0x00FF) | ((status as u16) << 8);
    }

    /// INT 14h, AH=02h - Read Character from Serial Port
    /// Input:
    ///   DX = port number
    /// Output:
    ///   AH = line status
    ///   AL = received character (if AH bit 7 = 0)
    fn int14_read_char(&mut self, port: u8, io: &mut super::Bios) {
        match io.serial_read(port) {
            Ok((ch, status)) => {
                // Character received successfully
                self.ax = ((status as u16) << 8) | (ch as u16);
            }
            Err(status) => {
                // Timeout or error - AH contains status with timeout bit set
                self.ax = (status as u16) << 8;
            }
        }
    }

    /// INT 14h, AH=03h - Get Serial Port Status
    /// Input:
    ///   DX = port number
    /// Output:
    ///   AH = line status
    ///   AL = modem status
    fn int14_get_status(&mut self, port: u8, io: &mut super::Bios) {
        let status = io.serial_status(port);

        // Set return values
        self.ax = ((status.line_status as u16) << 8) | (status.modem_status as u16);
    }
}
