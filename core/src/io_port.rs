use std::collections::HashMap;

/// Trait for I/O device implementations.
/// Allows platform-specific or custom I/O device behavior.
pub trait IoDevice {
    /// Read a byte from the specified I/O port.
    fn read_byte(&mut self, port: u16) -> u8;

    /// Write a byte to the specified I/O port.
    fn write_byte(&mut self, port: u16, value: u8);

    /// Read a word (16-bit) from the specified I/O port.
    /// Reads from port and port+1 in little-endian order.
    fn read_word(&mut self, port: u16) -> u16 {
        let low = self.read_byte(port);
        let high = self.read_byte(port.wrapping_add(1));
        (high as u16) << 8 | low as u16
    }

    /// Write a word (16-bit) to the specified I/O port.
    /// Writes to port and port+1 in little-endian order.
    fn write_word(&mut self, port: u16, value: u16) {
        let low = (value & 0xFF) as u8;
        let high = (value >> 8) as u8;
        self.write_byte(port, low);
        self.write_byte(port.wrapping_add(1), high);
    }
}

/// Null I/O device that returns 0xFF for all reads and ignores all writes.
/// Used as the default device for programs that don't need I/O.
#[derive(Default)]
pub struct NullIoDevice;

impl IoDevice for NullIoDevice {
    fn read_byte(&mut self, _port: u16) -> u8 {
        0xFF
    }

    fn write_byte(&mut self, _port: u16, _value: u8) {
        // Ignore writes
    }
}

/// I/O port manager.
/// Manages the 8086 I/O port address space (16-bit: 0x0000-0xFFFF).
pub struct IoPort<T: IoDevice = NullIoDevice> {
    /// Cache of port values (sparse storage)
    ports: HashMap<u16, u8>,
    /// I/O device handler
    device: T,
}

impl<T: IoDevice> IoPort<T> {
    /// Create a new I/O port manager with the specified device.
    pub fn new(device: T) -> Self {
        Self {
            ports: HashMap::new(),
            device,
        }
    }

    /// Read a byte from the specified port.
    /// Returns the cached value if available, otherwise queries the device.
    pub fn read_byte(&mut self, port: u16) -> u8 {
        if let Some(&value) = self.ports.get(&port) {
            value
        } else {
            self.device.read_byte(port)
        }
    }

    /// Write a byte to the specified port.
    /// Updates the cache and notifies the device.
    pub fn write_byte(&mut self, port: u16, value: u8) {
        self.ports.insert(port, value);
        self.device.write_byte(port, value);
    }

    /// Read a word (16-bit) from the specified port.
    /// Reads from port and port+1 in little-endian order.
    pub fn read_word(&mut self, port: u16) -> u16 {
        let low = self.read_byte(port);
        let high = self.read_byte(port.wrapping_add(1));
        (high as u16) << 8 | low as u16
    }

    /// Write a word (16-bit) to the specified port.
    /// Writes to port and port+1 in little-endian order.
    pub fn write_word(&mut self, port: u16, value: u16) {
        let low = (value & 0xFF) as u8;
        let high = (value >> 8) as u8;
        self.write_byte(port, low);
        self.write_byte(port.wrapping_add(1), high);
    }

    /// Get the cached value of a port for debugging/inspection.
    /// Returns None if the port has not been written to.
    pub fn get_port_value(&self, port: u16) -> Option<u8> {
        self.ports.get(&port).copied()
    }

    /// Reset all cached port values.
    pub fn reset(&mut self) {
        self.ports.clear();
    }
}
