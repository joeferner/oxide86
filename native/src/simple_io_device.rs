use emu86_core::IoDevice;
use log::info;
use std::collections::HashMap;

/// Simple I/O device implementation for native platform.
/// Provides basic port emulation for testing and debugging.
pub struct SimpleIoDevice {
    /// Track last written values for debugging
    last_write: HashMap<u16, u8>,
    /// Enable verbose logging of I/O operations
    verbose: bool,
}

impl SimpleIoDevice {
    /// Create a new SimpleIoDevice with optional verbose logging
    pub fn new(verbose: bool) -> Self {
        Self {
            last_write: HashMap::new(),
            verbose,
        }
    }
}

impl IoDevice for SimpleIoDevice {
    fn read_byte(&mut self, port: u16) -> u8 {
        let value = match port {
            // Keyboard controller - return dummy scancode
            0x60 => 0x1C, // 'a' key scancode

            // System control port - echo back last write
            0x61 => self.last_write.get(&port).copied().unwrap_or(0xFF),

            // All other ports return 0xFF (floating high)
            _ => self.last_write.get(&port).copied().unwrap_or(0xFF),
        };

        if self.verbose {
            info!("I/O Read:  Port 0x{:04X} -> 0x{:02X}", port, value);
        }

        value
    }

    fn write_byte(&mut self, port: u16, value: u8) {
        if self.verbose {
            info!("I/O Write: Port 0x{:04X} <- 0x{:02X}", port, value);
        }

        self.last_write.insert(port, value);
    }
}
