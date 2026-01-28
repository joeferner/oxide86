/// System Control Port (Port 61h) - 8255 PPI Port B
///
/// This port controls various system functions and reports status.
///
/// Bits 0-3 are writable control bits:
/// - Bit 0: Timer 2 gate (speaker gate)
/// - Bit 1: Speaker data enable
/// - Bit 2: Enable parity check
/// - Bit 3: Enable I/O channel check
///
/// Bits 4-7 are read-only status bits:
/// - Bit 4: Refresh request (toggles on each read to indicate memory refresh)
/// - Bit 5: Timer 2 output
/// - Bit 6: I/O channel check (0 = no error)
/// - Bit 7: Parity check (0 = no error)
pub struct SystemControlPort {
    /// Control bits (0-3) - writable
    control_bits: u8,
    /// Refresh toggle state for bit 4
    refresh_toggle: bool,
}

impl SystemControlPort {
    /// Create a new System Control Port with default state
    pub fn new() -> Self {
        Self {
            control_bits: 0,
            refresh_toggle: false,
        }
    }

    /// Read from port 61h
    /// Returns control bits (0-3) plus status bits (4-7)
    pub fn read(&mut self) -> u8 {
        // Toggle refresh bit on each read (simulates memory refresh)
        self.refresh_toggle = !self.refresh_toggle;

        // Build result byte
        let mut result = self.control_bits & 0x0F; // Control bits (0-3)

        if self.refresh_toggle {
            result |= 0x10; // Bit 4: Refresh toggle
        }

        // Bits 5-7 remain 0:
        // - Bit 5: Timer 2 output (not implemented, returns 0)
        // - Bit 6: I/O channel check (0 = no error)
        // - Bit 7: Parity check (0 = no error)

        result
    }

    /// Write to port 61h
    /// Only bits 0-3 are writable; bits 4-7 are read-only status
    pub fn write(&mut self, value: u8) {
        // Only store control bits (0-3), ignore writes to status bits (4-7)
        self.control_bits = value & 0x0F;
    }

    /// Reset the port to initial state
    pub fn reset(&mut self) {
        self.control_bits = 0;
        self.refresh_toggle = false;
    }
}

impl Default for SystemControlPort {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_toggle() {
        let mut port = SystemControlPort::new();

        // First read toggles false->true, so refresh bit is set (0x10)
        let val1 = port.read();
        assert_eq!(val1 & 0x10, 0x10);

        // Second read toggles true->false, so refresh bit is clear (0x00)
        let val2 = port.read();
        assert_eq!(val2 & 0x10, 0x00);

        // Third read toggles false->true, so refresh bit is set again (0x10)
        let val3 = port.read();
        assert_eq!(val3 & 0x10, 0x10);
    }

    #[test]
    fn test_control_bits() {
        let mut port = SystemControlPort::new();

        // Write control bits
        port.write(0x0F);
        assert_eq!(port.read() & 0x0F, 0x0F);

        // Write with status bits set (should be ignored)
        port.write(0xFF);
        assert_eq!(port.read() & 0x0F, 0x0F);
    }

    #[test]
    fn test_status_bits_read_only() {
        let mut port = SystemControlPort::new();

        // Try to write to status bits (4-7)
        port.write(0xF0);

        // Control bits should be 0
        let val = port.read();
        assert_eq!(val & 0x0F, 0x00);
    }
}
