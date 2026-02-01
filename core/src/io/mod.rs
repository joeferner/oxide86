mod pit;
mod system_control_port;

pub use pit::Pit;
use std::collections::HashMap;
pub use system_control_port::SystemControlPort;

/// I/O device implementation.
pub struct IoDevice {
    /// Track last written values for debugging
    last_write: HashMap<u16, u8>,
    /// System Control Port (port 61h)
    system_control_port: SystemControlPort,
    /// Programmable Interval Timer (ports 40h-43h)
    pit: Pit,
}

impl IoDevice {
    pub fn new() -> Self {
        Self {
            last_write: HashMap::new(),
            system_control_port: SystemControlPort::new(),
            pit: Pit::new(),
        }
    }

    /// Read a byte from the specified I/O port.
    pub fn read_byte(&mut self, port: u16) -> u8 {
        let value = match port {
            // PIT channel data ports
            0x40..=0x42 => self.pit.read_channel((port - 0x40) as u8),

            // PIT command port (write-only, return 0xFF on read)
            0x43 => 0xFF,

            // Keyboard controller - return dummy scancode
            0x60 => 0x1C, // 'a' key scancode

            // System control port with Timer 2 output
            0x61 => {
                let mut value = self.system_control_port.read();
                // Set bit 5 to reflect Timer 2 output state
                if self.pit.get_channel_output(2) {
                    value |= 0x20;
                }
                value
            }

            // All other ports return 0xFF (floating high)
            _ => self.last_write.get(&port).copied().unwrap_or(0xFF),
        };

        log::debug!("I/O Read:  Port 0x{:04X} -> 0x{:02X}", port, value);

        value
    }

    /// Write a byte to the specified I/O port.
    pub fn write_byte(&mut self, port: u16, value: u8) {
        log::debug!("I/O Write: Port 0x{:04X} <- 0x{:02X}", port, value);

        match port {
            // PIT channel data ports
            0x40..=0x42 => self.pit.write_channel((port - 0x40) as u8, value),

            // PIT command port
            0x43 => self.pit.write_command(value),

            // System control port
            0x61 => {
                self.system_control_port.write(value);
                // Update PIT Channel 2 gate from bit 0
                self.pit.set_gate(2, (value & 0x01) != 0);
            }

            _ => {}
        }

        self.last_write.insert(port, value);
    }

    /// Read a word (16-bit) from the specified I/O port.
    /// Reads from port and port+1 in little-endian order.
    pub fn read_word(&mut self, port: u16) -> u16 {
        let low = self.read_byte(port);
        let high = self.read_byte(port.wrapping_add(1));
        (high as u16) << 8 | low as u16
    }

    /// Write a word (16-bit) to the specified I/O port.
    /// Writes to port and port+1 in little-endian order.
    pub fn write_word(&mut self, port: u16, value: u16) {
        let low = (value & 0xFF) as u8;
        let high = (value >> 8) as u8;
        self.write_byte(port, low);
        self.write_byte(port.wrapping_add(1), high);
    }

    /// Update PIT counters based on CPU cycles
    /// Called from Computer::increment_cycles()
    pub fn update_pit(&mut self, cycles: u64) {
        self.pit.update(cycles);
    }
}
