use crate::cpu::cpu_flag;
use crate::memory::{
    BDA_KEYBOARD_BUFFER_HEAD, BDA_KEYBOARD_BUFFER_TAIL, BDA_KEYBOARD_FLAGS1, BDA_START,
};
use crate::{cpu::Cpu, memory::Memory};

impl Cpu {
    /// INT 0x16 - Keyboard Services
    /// AH register contains the function number
    pub(super) fn handle_int16(&mut self, memory: &mut Memory, io: &mut super::Bios) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int16_read_char(memory, io),
            0x01 => self.int16_check_keystroke(memory, io),
            0x02 => self.int16_get_shift_flags(memory),
            0x10 => self.int16_read_char(memory, io), // Extended read (same as 00h)
            0x11 => self.int16_check_keystroke(memory, io), // Extended check (same as 01h)
            0x12 => self.int16_get_shift_flags(memory), // Extended shift flags (same as 02h)
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
    pub(crate) fn int16_read_char(&mut self, memory: &mut Memory, io: &mut super::Bios) {
        // Check if there's already a character in the keyboard buffer
        let head_addr = BDA_START + BDA_KEYBOARD_BUFFER_HEAD;
        let tail_addr = BDA_START + BDA_KEYBOARD_BUFFER_TAIL;
        let head = memory.read_u16(head_addr);
        let tail = memory.read_u16(tail_addr);

        if head != tail {
            // Buffer has data - read from it
            let buffer_start = 0x001E; // Relative to BDA
            let char_addr = BDA_START + head as usize;
            let scan_code = memory.read_u8(char_addr);
            let ascii_code = memory.read_u8(char_addr + 1);

            log::debug!(
                "INT 16h AH=00h: Read key from buffer - Scan: 0x{:02X}, ASCII: 0x{:02X} ('{}')",
                scan_code,
                ascii_code,
                if (0x20..0x7F).contains(&ascii_code) {
                    ascii_code as char
                } else {
                    '.'
                }
            );

            // Update head pointer (circular buffer)
            let new_head = if head == buffer_start + 30 {
                buffer_start // Wrap around
            } else {
                head + 2
            };
            memory.write_u16(head_addr, new_head);

            // Set return values
            self.ax = ((scan_code as u16) << 8) | (ascii_code as u16);
        } else {
            // Buffer is empty - read from I/O
            if let Some(key) = io.read_key() {
                log::debug!(
                    "INT 16h AH=00h: Read key from I/O - Scan: 0x{:02X}, ASCII: 0x{:02X} ('{}')",
                    key.scan_code,
                    key.ascii_code,
                    if key.ascii_code >= 0x20 && key.ascii_code < 0x7F {
                        key.ascii_code as char
                    } else {
                        '.'
                    }
                );
                self.ax = ((key.scan_code as u16) << 8) | (key.ascii_code as u16);
            } else {
                // No key available - enter wait state
                // For blocking keyboards (terminal), read_key() should block internally and never return None
                // For non-blocking keyboards (GUI, WASM), we enter wait state and pause execution
                log::debug!("INT 16h AH=00h: No key available, entering wait state");
                self.set_waiting_for_keyboard();
                // Don't modify AX - when we resume, we need to re-execute this INT
                // The INT instruction is 2 bytes (0xCD nn), but we're being called after
                // the interrupt dispatch which has already advanced IP. We need to rewind
                // so the next execution retry happens at the instruction after the INT.
                // Actually, we'll handle the retry in the wait state resume logic.
            }
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
    fn int16_check_keystroke(&mut self, memory: &mut Memory, io: &mut super::Bios) {
        // Check keyboard buffer
        let head_addr = BDA_START + BDA_KEYBOARD_BUFFER_HEAD;
        let tail_addr = BDA_START + BDA_KEYBOARD_BUFFER_TAIL;
        let head = memory.read_u16(head_addr);
        let tail = memory.read_u16(tail_addr);

        if head != tail {
            // Buffer has data - peek at it without removing
            let char_addr = BDA_START + head as usize;
            let scan_code = memory.read_u8(char_addr);
            let ascii_code = memory.read_u8(char_addr + 1);

            log::debug!(
                "INT 16h AH=01h: Key available in buffer - Scan: 0x{:02X}, ASCII: 0x{:02X} ('{}')",
                scan_code,
                ascii_code,
                if (0x20..0x7F).contains(&ascii_code) {
                    ascii_code as char
                } else {
                    '.'
                }
            );

            // Set return values
            self.ax = ((scan_code as u16) << 8) | (ascii_code as u16);

            // Clear ZF to indicate keystroke available
            self.set_flag(cpu_flag::ZERO, false);
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
                    memory.write_u8(char_addr, key.scan_code);
                    memory.write_u8(char_addr + 1, key.ascii_code);
                    memory.write_u16(tail_addr, new_tail);

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
    fn int16_get_shift_flags(&mut self, memory: &Memory) {
        let flags_addr = BDA_START + BDA_KEYBOARD_FLAGS1;
        let flags = memory.read_u8(flags_addr);

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
