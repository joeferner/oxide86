use crate::Bus;
use crate::cpu::Cpu;
use crate::cpu::cpu_flag;
use crate::memory::{
    BDA_KEYBOARD_BUFFER_HEAD, BDA_KEYBOARD_BUFFER_TAIL, BDA_KEYBOARD_FLAGS1, BDA_START,
};

impl Cpu {
    pub(super) fn handle_int16(&mut self, bus: &mut Bus, io: &mut super::Bios) {
        match function {
            
            0x02 => self.int16_get_shift_flags(bus),
            0x10 => self.int16_read_char(bus, io), // Extended read (same as 00h)
            0x11 => self.int16_check_keystroke(bus, io), // Extended check (same as 01h)
            0x12 => self.int16_get_shift_flags(bus), // Extended shift flags (same as 02h)
            0x55 => self.int16_word_tsr_check(),
            0x92 => self.int16_get_keyboard_capabilities(),
            0xA2 => self.int16_122_key_capability_check(),
        }
    }

    /// INT 16h, AH=01h - Check for Keystroke
    /// Checks if a key is available without removing it
    /// Input: None
    /// Output:
    ///   ZF = 1 if no keystroke available
    ///   ZF = 0 if keystroke available
    ///   If keystroke available:
    ///     AH = BIOS scan code
    ///     AL = ASCII character
    fn int16_check_keystroke(&mut self, bus: &mut Bus, io: &mut super::Bios) {
        // MIGRATED

        if head != tail {
            // MIGRATED
        } else {
            // Buffer is empty - check if a key is available from the host (non-blocking)
            if let Some(key) = io.check_key() {
                log::debug!(
                    "INT 16h AH=01h: Key detected from I/O - Scan: 0x{:02X}, ASCII: 0x{:02X} ('{}'), adding to buffer",
                    key.scan_code,
                    key.ascii_code,
                    if key.ascii_code >= 0x20 && key.ascii_code < 0x7F {
                        key.ascii_code as char
                    } else {
                        '.'
                    }
                );
                // Calculate what tail would be after adding this key
                let buffer_start: u16 = 0x001E; // Relative to BDA
                let new_tail = if tail == buffer_start + 30 {
                    buffer_start // Wrap around
                } else {
                    tail + 2
                };

                // Check if buffer would become full (tail would catch up to head)
                // In a circular buffer, we sacrifice one slot to distinguish full from empty
                if new_tail == head {
                    // Buffer would be full - can't add more keys, but report key available
                    // (This shouldn't happen often, but prevents buffer corruption)
                    log::warn!(
                        "Keyboard buffer full! Scan: 0x{:02X}, ASCII: 0x{:02X}, head=0x{:04X}, tail=0x{:04X}",
                        key.scan_code,
                        key.ascii_code,
                        head,
                        tail
                    );
                    self.ax = ((key.scan_code as u16) << 8) | (key.ascii_code as u16);
                    self.set_flag(cpu_flag::ZERO, false);
                } else {
                    // Key is available and buffer has space - add it for later consumption
                    let char_addr = BDA_START + tail as usize;
                    bus.write_u8(char_addr, key.scan_code);
                    bus.write_u8(char_addr + 1, key.ascii_code);
                    bus.write_u16(tail_addr, new_tail);

                    // Return the key data
                    self.ax = ((key.scan_code as u16) << 8) | (key.ascii_code as u16);

                    // Clear ZF to indicate keystroke available
                    self.set_flag(cpu_flag::ZERO, false);
                }
            } else {
                // No keystroke available - set ZF
                self.set_flag(cpu_flag::ZERO, true);
            }
        }
    }

    /// INT 16h, AH=02h - Get Shift Flags
    /// Returns the current state of keyboard shift flags
    /// Input: None
    /// Output:
    ///   AL = shift flags
    ///     Bit 0: Right Shift pressed
    ///     Bit 1: Left Shift pressed
    ///     Bit 2: Ctrl pressed
    ///     Bit 3: Alt pressed
    ///     Bit 4: Scroll Lock active
    ///     Bit 5: Num Lock active
    ///     Bit 6: Caps Lock active
    ///     Bit 7: Insert mode active
    fn int16_get_shift_flags(&mut self, bus: &mut Bus) {
        let flags_addr = BDA_START + BDA_KEYBOARD_FLAGS1;
        let flags = bus.read_u8(flags_addr);

        // Return flags in AL
        self.ax = (self.ax & 0xFF00) | (flags as u16);
    }

    /// INT 16h, AH=92h - Get Keyboard Capabilities Flag
    /// Returns keyboard capabilities, indicating support for INT 15h keyboard intercept
    /// Input: None
    /// Output:
    ///   AH = capabilities flag
    ///     Bit 7: INT 15h, AH=4Fh keyboard intercept supported
    ///   CF = clear (success)
    fn int16_get_keyboard_capabilities(&mut self) {
        // Return AH with bit 7 set to indicate INT 15h keyboard intercept is supported
        self.ax = (self.ax & 0x00FF) | 0x8000;
        self.set_flag(cpu_flag::CARRY, false);
    }

    /// INT 16h, AH=55h - Microsoft Word TSR Detection
    /// Used by Microsoft Word for DOS to detect if its TSR utility is installed.
    /// If the TSR were installed, it would return "MS" (0x4D53) in AX.
    /// Input: None
    /// Output: AX unchanged (signals TSR NOT installed)
    fn int16_word_tsr_check(&mut self) {
        // Intentionally do nothing - leave AX unchanged to indicate
        // that the Microsoft Word TSR is NOT installed
    }

    /// INT 16h, AH=A2h - 122-Key Keyboard Capability Check
    /// Called by DOS 5.0+ KEYB.COM to detect support for 122-key keyboard functions.
    /// This is not a real BIOS function but a detection mechanism that exploits how
    /// the BIOS handles unknown function numbers. KEYB.COM checks if AH is modified
    /// to determine if functions 20h-22h are supported.
    /// Input: None
    /// Output: AH unchanged (signals functions 20h-22h NOT supported)
    fn int16_122_key_capability_check(&mut self) {
        // Intentionally do nothing - leave AH unchanged to indicate
        // that 122-key keyboard functions (AH=20h-22h) are NOT supported
    }
}
