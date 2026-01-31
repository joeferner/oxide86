use std::collections::VecDeque;

// UART Register offsets (from base port)
pub mod register {
    pub const DATA: u16 = 0; // Receive/Transmit Buffer (or Divisor Latch Low when DLAB=1)
    pub const INTERRUPT_ENABLE: u16 = 1; // Interrupt Enable Register (or Divisor Latch High when DLAB=1)
    pub const INTERRUPT_ID: u16 = 2; // Interrupt Identification Register
    pub const LINE_CONTROL: u16 = 3; // Line Control Register (includes DLAB bit)
    pub const MODEM_CONTROL: u16 = 4; // Modem Control Register
    pub const LINE_STATUS: u16 = 5; // Line Status Register
    pub const MODEM_STATUS: u16 = 6; // Modem Status Register
    pub const SCRATCH: u16 = 7; // Scratch Register
}

// Line Status Register bits
pub mod line_status {
    pub const DATA_READY: u8 = 0x01; // Bit 0: Data ready
    pub const OVERRUN_ERROR: u8 = 0x02; // Bit 1: Overrun error
    pub const PARITY_ERROR: u8 = 0x04; // Bit 2: Parity error
    pub const FRAMING_ERROR: u8 = 0x08; // Bit 3: Framing error
    pub const BREAK_INTERRUPT: u8 = 0x10; // Bit 4: Break interrupt
    pub const TRANSMIT_HOLDING_EMPTY: u8 = 0x20; // Bit 5: THR empty
    pub const TRANSMIT_SHIFT_EMPTY: u8 = 0x40; // Bit 6: TSR empty
    pub const TIMEOUT: u8 = 0x80; // Bit 7: Timeout error
}

// Modem Status Register bits
pub mod modem_status {
    pub const DELTA_CLEAR_TO_SEND: u8 = 0x01; // Bit 0: DCTS
    pub const DELTA_DATA_SET_READY: u8 = 0x02; // Bit 1: DDSR
    pub const TRAILING_EDGE_RING: u8 = 0x04; // Bit 2: TERI
    pub const DELTA_CARRIER_DETECT: u8 = 0x08; // Bit 3: DDCD
    pub const CLEAR_TO_SEND: u8 = 0x10; // Bit 4: CTS
    pub const DATA_SET_READY: u8 = 0x20; // Bit 5: DSR
    pub const RING_INDICATOR: u8 = 0x40; // Bit 6: RI
    pub const CARRIER_DETECT: u8 = 0x80; // Bit 7: DCD
}

// Line Control Register bits
pub mod line_control {
    pub const DLAB: u8 = 0x80; // Bit 7: Divisor Latch Access Bit
}

/// Serial port parameters (from INT 14h AH=00h)
#[derive(Debug, Clone, Copy)]
pub struct SerialParams {
    pub baud_rate: u8, // Bits 7-5: 000=110, 001=150, 010=300, 011=600, 100=1200, 101=2400, 110=4800, 111=9600
    pub parity: u8,    // Bits 4-3: 00=none, 01=odd, 10=none, 11=even
    pub stop_bits: u8, // Bit 2: 0=1 stop bit, 1=2 stop bits
    pub word_length: u8, // Bits 1-0: 10=7 bits, 11=8 bits
}

impl SerialParams {
    pub fn from_int14_al(al: u8) -> Self {
        Self {
            baud_rate: (al >> 5) & 0x07,
            parity: (al >> 3) & 0x03,
            stop_bits: (al >> 2) & 0x01,
            word_length: al & 0x03,
        }
    }
}

impl Default for SerialParams {
    fn default() -> Self {
        Self {
            baud_rate: 0x06,   // 4800 baud
            parity: 0x00,      // No parity
            stop_bits: 0x00,   // 1 stop bit
            word_length: 0x03, // 8 bits
        }
    }
}

/// Serial port status (returned by INT 14h)
#[derive(Debug, Clone, Copy)]
pub struct SerialStatus {
    pub line_status: u8,
    pub modem_status: u8,
}

/// Trait for devices that can be attached to serial ports
pub trait SerialDevice {
    /// Called when the port is initialized with new parameters
    /// Returns optional initialization response bytes (e.g., "M" for Microsoft Mouse)
    fn on_init(&mut self, params: &SerialParams) -> Option<Vec<u8>>;

    /// Called periodically to allow device to generate data
    /// Returns bytes to enqueue into the RX buffer
    fn update(&mut self) -> Vec<u8>;

    /// Called when a byte is written to the serial port
    /// Allows device to respond to commands
    fn on_write(&mut self, byte: u8);
}

/// 16450 UART Serial Port Controller
pub struct SerialPortController {
    port_number: u8, // 0=COM1, 1=COM2
    base_port: u16,  // 0x3F8 (COM1) or 0x2F8 (COM2)

    // Buffers
    rx_buffer: VecDeque<u8>,
    tx_buffer: VecDeque<u8>,

    // UART Registers (16450 compatible)
    interrupt_enable: u8, // IER - Interrupt Enable Register
    line_control: u8,     // LCR - Line Control Register (includes DLAB bit 7)
    modem_control: u8,    // MCR - Modem Control Register
    line_status: u8,      // LSR - Line Status Register (bit 0=data ready)
    pub modem_status: u8, // MSR - Modem Status Register
    scratch: u8,          // Scratch register
    divisor_latch: u16,   // Baud rate divisor (accessible when DLAB=1)

    // Configuration
    pub params: SerialParams,
    buffer_size: usize,

    // Attached device
    device: Option<Box<dyn SerialDevice>>,

    // Interrupt flag
    pending_interrupt: bool,
}

impl SerialPortController {
    /// Create a new serial port controller
    /// port: 0=COM1, 1=COM2
    pub fn new(port: u8) -> Self {
        let base_port = match port {
            0 => 0x3F8, // COM1
            1 => 0x2F8, // COM2
            _ => 0x3F8, // Default to COM1
        };

        Self {
            port_number: port,
            base_port,
            rx_buffer: VecDeque::with_capacity(256),
            tx_buffer: VecDeque::with_capacity(256),
            interrupt_enable: 0,
            line_control: 0x03, // Default: 8N1
            modem_control: 0,
            line_status: line_status::TRANSMIT_HOLDING_EMPTY | line_status::TRANSMIT_SHIFT_EMPTY,
            modem_status: modem_status::DATA_SET_READY | modem_status::CLEAR_TO_SEND,
            scratch: 0,
            divisor_latch: 96, // Default to 1200 baud (115200 / 1200 = 96)
            params: SerialParams::default(),
            buffer_size: 256,
            device: None,
            pending_interrupt: false,
        }
    }

    /// Enqueue a byte to the receive buffer (called by mouse/serial device)
    /// Returns true if successful, false if buffer full
    pub fn enqueue_byte(&mut self, byte: u8) -> bool {
        if self.rx_buffer.len() >= self.buffer_size {
            // Buffer overflow - set overrun error
            self.line_status |= line_status::OVERRUN_ERROR;
            return false;
        }

        self.rx_buffer.push_back(byte);
        self.update_line_status();

        // Check if "Received Data Available" interrupt is enabled (bit 0 of IER)
        if self.interrupt_enable & 0x01 != 0 {
            self.pending_interrupt = true;
            log::debug!(
                "COM{}: Data arrived, interrupt enabled, setting pending_interrupt flag",
                self.port_number + 1
            );
        }

        true
    }

    /// Dequeue a byte from the receive buffer (called by INT 14h or I/O port read)
    /// Returns the byte if available, None otherwise
    pub fn dequeue_byte(&mut self) -> Option<u8> {
        let byte = self.rx_buffer.pop_front();
        self.update_line_status();
        byte
    }

    /// Read a UART register
    /// offset: 0-7 (register offset from base port)
    pub fn read_register(&mut self, offset: u16) -> u8 {
        match offset {
            register::DATA => {
                // Check DLAB bit
                if self.line_control & line_control::DLAB != 0 {
                    // DLAB=1: Return divisor latch low byte
                    (self.divisor_latch & 0xFF) as u8
                } else {
                    // DLAB=0: Read and consume byte from receive buffer
                    let byte = self.dequeue_byte().unwrap_or(0);

                    // If more data remains in buffer and interrupts enabled, re-trigger interrupt
                    if !self.rx_buffer.is_empty() && (self.interrupt_enable & 0x01 != 0) {
                        self.pending_interrupt = true;
                        log::debug!(
                            "COM{}: Data still in buffer after read, re-triggering interrupt",
                            self.port_number + 1
                        );
                    }

                    byte
                }
            }
            register::INTERRUPT_ENABLE => {
                // Check DLAB bit
                if self.line_control & line_control::DLAB != 0 {
                    // DLAB=1: Return divisor latch high byte
                    ((self.divisor_latch >> 8) & 0xFF) as u8
                } else {
                    // DLAB=0: Return interrupt enable register
                    self.interrupt_enable
                }
            }
            register::INTERRUPT_ID => {
                // Interrupt Identification Register
                0x01 // No interrupt pending
            }
            register::LINE_CONTROL => self.line_control,
            register::MODEM_CONTROL => self.modem_control,
            register::LINE_STATUS => self.line_status,
            register::MODEM_STATUS => self.modem_status,
            register::SCRATCH => self.scratch,
            _ => 0xFF,
        }
    }

    /// Write a UART register
    /// offset: 0-7 (register offset from base port)
    pub fn write_register(&mut self, offset: u16, value: u8) {
        match offset {
            register::DATA => {
                // Check DLAB bit
                if self.line_control & line_control::DLAB != 0 {
                    // DLAB=1: Set divisor latch low byte
                    self.divisor_latch = (self.divisor_latch & 0xFF00) | (value as u16);
                    self.update_params_from_divisor();
                } else {
                    // DLAB=0: Write to transmit buffer
                    if self.tx_buffer.len() < self.buffer_size {
                        self.tx_buffer.push_back(value);
                    }
                }
            }
            register::INTERRUPT_ENABLE => {
                // Check DLAB bit
                if self.line_control & line_control::DLAB != 0 {
                    // DLAB=1: Set divisor latch high byte
                    self.divisor_latch = (self.divisor_latch & 0x00FF) | ((value as u16) << 8);
                    self.update_params_from_divisor();
                } else {
                    // DLAB=0: Set interrupt enable register
                    self.interrupt_enable = value;
                }
            }
            register::LINE_CONTROL => {
                self.line_control = value;
                self.update_params_from_line_control();
            }
            register::MODEM_CONTROL => {
                // Detect DTR transition (bit 0) - Microsoft Serial Mouse resets on DTR high
                let old_dtr = self.modem_control & 0x01;
                let new_dtr = value & 0x01;

                self.modem_control = value;

                // If DTR transitioned from 0 to 1, trigger device initialization
                if old_dtr == 0
                    && new_dtr == 1
                    && let Some(ref mut device) = self.device
                    && let Some(response) = device.on_init(&self.params)
                {
                    for byte in response {
                        self.enqueue_byte(byte);
                    }
                }
            }
            register::SCRATCH => {
                self.scratch = value;
            }
            _ => {
                // Other registers are read-only or not implemented
            }
        }
    }

    /// Get the line status register value
    pub fn get_line_status(&self) -> u8 {
        self.line_status
    }

    /// Update line status register based on buffer state
    fn update_line_status(&mut self) {
        // Update DATA_READY bit based on RX buffer
        if self.rx_buffer.is_empty() {
            self.line_status &= !line_status::DATA_READY;
        } else {
            self.line_status |= line_status::DATA_READY;
        }

        // Transmitter is always ready in our emulation
        self.line_status |= line_status::TRANSMIT_HOLDING_EMPTY | line_status::TRANSMIT_SHIFT_EMPTY;
    }

    /// Update params.baud_rate based on current divisor_latch value
    /// Divisor = 115200 / baud_rate
    fn update_params_from_divisor(&mut self) {
        let old_baud = self.params.baud_rate;
        self.params.baud_rate = match self.divisor_latch {
            1047 => 0x00, // 110 baud
            768 => 0x01,  // 150 baud
            384 => 0x02,  // 300 baud
            192 => 0x03,  // 600 baud
            96 => 0x04,   // 1200 baud
            48 => 0x05,   // 2400 baud
            24 => 0x06,   // 4800 baud
            12 => 0x07,   // 9600 baud
            _ => {
                // For other divisors, approximate to nearest standard rate
                let baud = 115200u32 / (self.divisor_latch as u32).max(1);
                if baud >= 7200 {
                    0x07 // 9600
                } else if baud >= 3600 {
                    0x06 // 4800
                } else if baud >= 1800 {
                    0x05 // 2400
                } else if baud >= 900 {
                    0x04 // 1200
                } else if baud >= 450 {
                    0x03 // 600
                } else if baud >= 225 {
                    0x02 // 300
                } else if baud >= 130 {
                    0x01 // 150
                } else {
                    0x00 // 110
                }
            }
        };

        if self.params.baud_rate != old_baud {
            let baud_name = match self.params.baud_rate {
                0x00 => "110",
                0x01 => "150",
                0x02 => "300",
                0x03 => "600",
                0x04 => "1200",
                0x05 => "2400",
                0x06 => "4800",
                0x07 => "9600",
                _ => "unknown",
            };
            log::debug!(
                "COM{} baud rate changed: divisor=0x{:04X} -> {} baud (code 0x{:02X})",
                self.port_number + 1,
                self.divisor_latch,
                baud_name,
                self.params.baud_rate
            );
        }
    }

    /// Update params from line control register
    /// Bits 1-0: word length (10=7 bits, 11=8 bits)
    /// Bit 2: stop bits (0=1, 1=2)
    /// Bits 4-3: parity (00=none, 01=odd, 11=even)
    fn update_params_from_line_control(&mut self) {
        let old_word_length = self.params.word_length;
        let old_stop_bits = self.params.stop_bits;
        let old_parity = self.params.parity;

        self.params.word_length = self.line_control & 0x03;
        self.params.stop_bits = (self.line_control >> 2) & 0x01;
        self.params.parity = (self.line_control >> 3) & 0x03;

        if self.params.word_length != old_word_length
            || self.params.stop_bits != old_stop_bits
            || self.params.parity != old_parity
        {
            let data_bits = match self.params.word_length {
                0x02 => "7",
                0x03 => "8",
                _ => "?",
            };
            let parity = match self.params.parity {
                0x00 => "N",
                0x01 => "O",
                0x03 => "E",
                _ => "?",
            };
            let stop_bits = if self.params.stop_bits == 0 { "1" } else { "2" };

            log::debug!(
                "COM{} line format changed: {}{}{}",
                self.port_number + 1,
                data_bits,
                parity,
                stop_bits
            );
        }
    }

    /// Reset the serial port to initial state
    pub fn reset(&mut self) {
        self.rx_buffer.clear();
        self.tx_buffer.clear();
        self.interrupt_enable = 0;
        self.line_control = 0x03; // 8N1
        self.modem_control = 0;
        self.line_status = line_status::TRANSMIT_HOLDING_EMPTY | line_status::TRANSMIT_SHIFT_EMPTY;
        self.modem_status = modem_status::DATA_SET_READY | modem_status::CLEAR_TO_SEND;
        self.scratch = 0;
        self.divisor_latch = 96;
        self.params = SerialParams::default();
        self.pending_interrupt = false;
    }

    /// Get the base I/O port address
    pub fn get_base_port(&self) -> u16 {
        self.base_port
    }

    /// Get the port number (0=COM1, 1=COM2)
    pub fn get_port_number(&self) -> u8 {
        self.port_number
    }

    /// Check if an interrupt is pending
    pub fn has_pending_interrupt(&self) -> bool {
        self.pending_interrupt
    }

    /// Clear the pending interrupt flag (should be called after firing the IRQ)
    pub fn clear_pending_interrupt(&mut self) {
        self.pending_interrupt = false;
    }

    /// Attach a device to this serial port
    pub fn attach_device(&mut self, device: Box<dyn SerialDevice>) {
        self.device = Some(device);
    }

    /// Detach the current device
    pub fn detach_device(&mut self) {
        self.device = None;
        self.rx_buffer.clear();
    }

    /// Update attached device and queue any generated bytes
    pub fn update_device(&mut self) {
        if let Some(ref mut device) = self.device {
            let bytes = device.update();
            for byte in bytes {
                self.enqueue_byte(byte);
            }
        }
    }

    /// Call when port is initialized
    pub fn on_init(&mut self, params: SerialParams) {
        self.params = params;
        if let Some(ref mut device) = self.device
            && let Some(response) = device.on_init(&params)
        {
            for byte in response {
                self.enqueue_byte(byte);
            }
        }
    }

    /// Call when byte is written to port
    pub fn on_write(&mut self, byte: u8) {
        if let Some(ref mut device) = self.device {
            device.on_write(byte);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_port_creation() {
        let port = SerialPortController::new(0);
        assert_eq!(port.get_base_port(), 0x3F8);
        assert_eq!(port.get_port_number(), 0);

        let port2 = SerialPortController::new(1);
        assert_eq!(port2.get_base_port(), 0x2F8);
        assert_eq!(port2.get_port_number(), 1);
    }

    #[test]
    fn test_enqueue_dequeue() {
        let mut port = SerialPortController::new(0);

        // Initially empty
        assert!(port.dequeue_byte().is_none());
        assert_eq!(port.get_line_status() & line_status::DATA_READY, 0);

        // Enqueue byte
        assert!(port.enqueue_byte(0x42));
        assert_eq!(
            port.get_line_status() & line_status::DATA_READY,
            line_status::DATA_READY
        );

        // Dequeue byte
        assert_eq!(port.dequeue_byte(), Some(0x42));
        assert_eq!(port.get_line_status() & line_status::DATA_READY, 0);
    }

    #[test]
    fn test_buffer_overflow() {
        let mut port = SerialPortController::new(0);

        // Fill buffer
        for i in 0..256 {
            assert!(port.enqueue_byte(i as u8));
        }

        // Buffer full - should fail
        assert!(!port.enqueue_byte(0xFF));
        assert_eq!(
            port.get_line_status() & line_status::OVERRUN_ERROR,
            line_status::OVERRUN_ERROR
        );
    }

    #[test]
    fn test_dlab_divisor_latch() {
        let mut port = SerialPortController::new(0);

        // Set DLAB bit
        port.write_register(register::LINE_CONTROL, line_control::DLAB | 0x03);

        // Write divisor latch (low then high)
        port.write_register(register::DATA, 0x60); // Low byte = 96
        port.write_register(register::INTERRUPT_ENABLE, 0x00); // High byte = 0

        assert_eq!(port.divisor_latch, 96);

        // Read back divisor latch
        assert_eq!(port.read_register(register::DATA), 0x60);
        assert_eq!(port.read_register(register::INTERRUPT_ENABLE), 0x00);

        // Clear DLAB bit - should access data/IER now
        port.write_register(register::LINE_CONTROL, 0x03);
        port.enqueue_byte(0x42);
        assert_eq!(port.read_register(register::DATA), 0x42);
    }

    #[test]
    fn test_serial_params() {
        // 1200 baud (100), no parity (00), 1 stop (0), 7 bits (10)
        let params = SerialParams::from_int14_al(0b10000010);
        assert_eq!(params.baud_rate, 0x04);
        assert_eq!(params.parity, 0x00);
        assert_eq!(params.stop_bits, 0x00);
        assert_eq!(params.word_length, 0x02);
    }
}
