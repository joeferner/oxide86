use crate::{
    bus::Bus,
    byte_to_printable_char,
    cpu::{
        Cpu,
        bios::bda::{bda_peek_key, bda_read_key},
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
        // Enable interrupts during keyboard operations (AT-class BIOS behavior)
        // This allows keyboard and timer IRQs to fire even when programs poll with IF=0
        self.set_flag(cpu_flag::INTERRUPT, true);

        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int16_read_char(bus),
            0x01 => self.int16_check_keystroke(bus),
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
}
