use emu86_core::{IoDevice, io::SystemControlPort};
use std::collections::HashMap;

/// Simple I/O device implementation for native platform.
/// Provides basic port emulation for testing and debugging.
pub struct SimpleIoDevice {
    /// Track last written values for debugging
    last_write: HashMap<u16, u8>,
    /// Enable verbose logging of I/O operations
    verbose: bool,
    /// System Control Port (port 61h)
    system_control_port: SystemControlPort,
}

impl SimpleIoDevice {
    /// Create a new SimpleIoDevice with optional verbose logging
    pub fn new(verbose: bool) -> Self {
        Self {
            last_write: HashMap::new(),
            verbose,
            system_control_port: SystemControlPort::new(),
        }
    }
}

impl IoDevice for SimpleIoDevice {
    fn read_byte(&mut self, port: u16) -> u8 {
        let value = match port {
            // Keyboard controller - return dummy scancode
            0x60 => 0x1C, // 'a' key scancode

            // System control port - delegate to SystemControlPort
            0x61 => self.system_control_port.read(),

            // All other ports return 0xFF (floating high)
            _ => self.last_write.get(&port).copied().unwrap_or(0xFF),
        };

        if self.verbose {
            log::info!("I/O Read:  Port 0x{:04X} -> 0x{:02X}", port, value);
        }

        value
    }

    fn write_byte(&mut self, port: u16, value: u8) {
        if self.verbose {
            log::info!("I/O Write: Port 0x{:04X} <- 0x{:02X}", port, value);
        }

        // Handle system control port specifically
        if port == 0x61 {
            self.system_control_port.write(value);
        }

        self.last_write.insert(port, value);
    }
}
