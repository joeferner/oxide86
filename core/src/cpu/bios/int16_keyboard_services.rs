use crate::{
    bus::Bus,
    byte_to_printable_char,
    cpu::{
        Cpu,
        bios::bda::{bda_get_keyboard_flags1, bda_peek_key, bda_read_key},
        cpu_flag,
    },
};

impl Cpu {
    /// INT 0x16 - Keyboard Services
    /// AH register contains the function number
    ///
    /// Note: Like INT 0x13 and INT 0x1A, we enable interrupts (STI) during keyboard
    /// services so that keyboard IRQs (INT 0x09) and timer IRQs (INT 0x08) can fire.
    /// This is important for programs that poll keyboard status in tight loops with
    /// interrupts disabled, or block waiting for input via AH=00h.
    pub(in crate::cpu) fn handle_int16_keyboard_services(&mut self, bus: &mut Bus) {
        bus.increment_cycle_count(300);
        // Enable interrupts during keyboard operations (AT-class BIOS behavior)
        // This allows keyboard and timer IRQs to fire even when programs poll with IF=0
        self.set_flag(cpu_flag::INTERRUPT, true);

        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int16_read_char(bus),
            0x01 => self.int16_check_keystroke(bus),
            0x02 => self.int16_get_shift_flags(bus),
            0x55 => self.int16_word_tsr_check(),
            0x92 => self.int16_get_keyboard_capabilities(),
            0xA2 => self.int16_122_key_capability_check(),
            _ => {
                log::warn!("Unhandled INT 0x16 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 16h, AH=00h - Read Character
    /// Waits for a keypress and returns it
    /// Input: None
    /// Output:
    ///   AH = BIOS scan code
    ///   AL = ASCII character
    pub(crate) fn int16_read_char(&mut self, bus: &mut Bus) {
        if let Some(key) = bda_read_key(bus) {
            log::debug!(
                "int16 read char scan code: 0x{:02X} '{}'",
                key.scan_code,
                byte_to_printable_char(key.ascii_code)
            );
            self.ax = ((key.scan_code as u16) << 8) | (key.ascii_code as u16);
        } else {
            self.wait_for_key_press_patch_flags = false;
            self.wait_for_key_press = true;
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
    fn int16_check_keystroke(&mut self, bus: &mut Bus) {
        if let Some(key) = bda_peek_key(bus) {
            log::debug!(
                "INT 0x16 AH=0x01: Key available in buffer - Scan: 0x{:02X}, ASCII: 0x{:02X} ('{}')",
                key.scan_code,
                key.ascii_code,
                byte_to_printable_char(key.ascii_code)
            );

            // Set return values
            self.ax = ((key.scan_code as u16) << 8) | (key.ascii_code as u16);

            // Clear ZF to indicate keystroke available
            self.set_flag(cpu_flag::ZERO, false);
        } else {
            // Set ZF to indicate no keystroke available
            self.set_flag(cpu_flag::ZERO, true);
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
    fn int16_get_shift_flags(&mut self, bus: &Bus) {
        let flags = bda_get_keyboard_flags1(bus);

        // Return flags in AL
        self.ax = (self.ax & 0xFF00) | (flags as u16);
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
