mod cga_ports;
mod pit;
mod system_control_port;

use crate::video::Video;
use cga_ports::CgaModeControl;
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
    /// CGA Mode Control Register (port 3D8h)
    cga_mode_control: CgaModeControl,
    /// Keyboard controller data port (port 60h) - stores last scan code
    keyboard_scan_code: u8,
    /// ASCII code corresponding to the last scan code (for BIOS INT 09h handler)
    keyboard_ascii_code: u8,
}

impl IoDevice {
    pub fn new() -> Self {
        Self {
            last_write: HashMap::new(),
            system_control_port: SystemControlPort::new(),
            pit: Pit::new(),
            cga_mode_control: CgaModeControl::new(),
            keyboard_scan_code: 0x00,
            keyboard_ascii_code: 0x00,
        }
    }

    /// Read a byte from the specified I/O port.
    pub fn read_byte(&mut self, port: u16) -> u8 {
        let value = match port {
            // PIT channel data ports
            0x40..=0x42 => self.pit.read_channel((port - 0x40) as u8),

            // PIT command port (write-only, return 0xFF on read)
            0x43 => 0xFF,

            // Keyboard controller data port - return last scan code
            0x60 => self.keyboard_scan_code,

            // System control port with Timer 2 output
            0x61 => {
                let mut value = self.system_control_port.read();
                // Set bit 5 to reflect Timer 2 output state
                if self.pit.get_channel_output(2) {
                    value |= 0x20;
                }
                value
            }

            // CGA Mode Control Register (read-only in practice)
            0x3D8 => self.cga_mode_control.read(),

            // CGA Color Select Register (write-only, return 0xFF on read)
            0x3D9 => 0xFF,

            // All other ports return 0xFF (floating high)
            _ => self.last_write.get(&port).copied().unwrap_or(0xFF),
        };

        log::trace!("I/O Read:  Port 0x{:04X} -> 0x{:02X}", port, value);

        value
    }

    /// Write a byte to the specified I/O port.
    pub fn write_byte(&mut self, port: u16, value: u8, video: &mut Video) {
        log::trace!("I/O Write: Port 0x{:04X} <- 0x{:02X}", port, value);

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

            // CGA Mode Control Register
            0x3D8 => {
                self.cga_mode_control.write(value);
                log::debug!("CGA Mode Control: 0x{:02X}", value);
            }

            // CGA Color Select Register
            0x3D9 => {
                video.set_palette(value);
                log::debug!("CGA Color Select: 0x{:02X}", value);
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
    pub fn write_word(&mut self, port: u16, value: u16, video: &mut Video) {
        let low = (value & 0xFF) as u8;
        let high = (value >> 8) as u8;
        self.write_byte(port, low, video);
        self.write_byte(port.wrapping_add(1), high, video);
    }

    /// Update PIT counters based on CPU cycles
    /// Called from Computer::increment_cycles()
    pub fn update_pit(&mut self, cycles: u64) {
        self.pit.update(cycles);
    }

    /// Get reference to the PIT (for speaker integration)
    pub fn pit(&self) -> &Pit {
        &self.pit
    }

    /// Get reference to the system control port (for speaker integration)
    pub fn system_control_port(&self) -> &SystemControlPort {
        &self.system_control_port
    }

    /// Set the keyboard scan code and ASCII code for INT 09h
    /// Called when firing keyboard IRQ
    pub fn set_keyboard_data(&mut self, scan_code: u8, ascii_code: u8) {
        self.keyboard_scan_code = scan_code;
        self.keyboard_ascii_code = ascii_code;
    }

    /// Get the keyboard ASCII code (for BIOS INT 09h handler)
    pub fn get_keyboard_ascii_code(&self) -> u8 {
        self.keyboard_ascii_code
    }
}
