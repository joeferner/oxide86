use crate::serial_port::{SerialDevice, SerialParams};

/// Serial port logger device that captures output and writes it to the log
pub struct SerialLogger {
    port_name: String,
    line_buffer: Vec<u8>,
    initialized: bool,
}

impl SerialLogger {
    /// Create a new serial logger
    /// port_number: 0=COM1, 1=COM2
    pub fn new(port_number: u8) -> Self {
        let port_name = format!("COM{}", port_number + 1);
        Self {
            port_name,
            line_buffer: Vec::new(),
            initialized: false,
        }
    }

    /// Flush the current line buffer to the log
    fn flush_line(&mut self) {
        if self.line_buffer.is_empty() {
            return;
        }

        // Convert buffer to string, handling non-UTF8 bytes gracefully
        let line = String::from_utf8_lossy(&self.line_buffer);
        log::info!("[{}] {}", self.port_name, line);

        self.line_buffer.clear();
    }
}

impl SerialDevice for SerialLogger {
    fn on_init(&mut self, params: &SerialParams) -> Option<Vec<u8>> {
        self.initialized = true;

        let baud_name = match params.baud_rate {
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

        let data_bits = match params.word_length {
            0x02 => "7",
            0x03 => "8",
            _ => "?",
        };

        let parity = match params.parity {
            0x00 => "N",
            0x01 => "O",
            0x03 => "E",
            _ => "?",
        };

        let stop_bits = if params.stop_bits == 0 { "1" } else { "2" };

        log::debug!(
            "{}: Initialized - {} baud, {}{}{}",
            self.port_name,
            baud_name,
            data_bits,
            parity,
            stop_bits
        );

        // Logger doesn't send response bytes to CPU
        None
    }

    fn update(&mut self) -> Vec<u8> {
        // Logger doesn't generate data for CPU to read
        Vec::new()
    }

    fn on_write(&mut self, byte: u8) {
        if !self.initialized {
            return;
        }

        // Handle line endings
        if byte == b'\n' {
            // Newline - flush the line
            self.flush_line();
        } else if byte == b'\r' {
            // Carriage return - flush if buffer has content
            // (handles both \r and \r\n line endings)
            if !self.line_buffer.is_empty() {
                self.flush_line();
            }
        } else {
            // Regular character - add to buffer
            self.line_buffer.push(byte);
        }
    }

    fn on_port_reset(&mut self) {
        log::debug!("{}: Port reset", self.port_name);

        // Flush any remaining buffered data
        if !self.line_buffer.is_empty() {
            self.flush_line();
        }

        self.initialized = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_creation() {
        let logger = SerialLogger::new(0);
        assert_eq!(logger.port_name, "COM1");
        assert!(!logger.initialized);

        let logger2 = SerialLogger::new(1);
        assert_eq!(logger2.port_name, "COM2");
    }

    #[test]
    fn test_line_buffering() {
        let mut logger = SerialLogger::new(0);

        // Initialize first
        let params = SerialParams::default();
        logger.on_init(&params);

        // Write "Hello\n"
        for &b in b"Hello" {
            logger.on_write(b);
        }
        assert_eq!(logger.line_buffer, b"Hello");

        // Newline should flush
        logger.on_write(b'\n');
        assert!(logger.line_buffer.is_empty());
    }

    #[test]
    fn test_crlf_handling() {
        let mut logger = SerialLogger::new(0);
        let params = SerialParams::default();
        logger.on_init(&params);

        // Write "Hello\r\n"
        for &b in b"Hello" {
            logger.on_write(b);
        }
        logger.on_write(b'\r');
        assert!(logger.line_buffer.is_empty()); // \r flushes

        logger.on_write(b'\n'); // \n on empty buffer does nothing
        assert!(logger.line_buffer.is_empty());
    }

    #[test]
    fn test_reset_flushes_buffer() {
        let mut logger = SerialLogger::new(0);
        let params = SerialParams::default();
        logger.on_init(&params);

        // Write partial line
        for &b in b"Partial" {
            logger.on_write(b);
        }
        assert_eq!(logger.line_buffer, b"Partial");

        // Reset should flush and clear initialized flag
        logger.on_port_reset();
        assert!(logger.line_buffer.is_empty());
        assert!(!logger.initialized);
    }
}
