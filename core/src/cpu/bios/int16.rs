use crate::cpu::cpu_flag;
use crate::memory::{
    BDA_KEYBOARD_BUFFER_HEAD, BDA_KEYBOARD_BUFFER_TAIL, BDA_KEYBOARD_FLAGS1, BDA_START,
};
use crate::{cpu::Cpu, memory::Memory};

impl Cpu {
    /// INT 0x16 - Keyboard Services
    /// AH register contains the function number
    pub(super) fn handle_int16<T: super::Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int16_read_char(memory, io),
            0x01 => self.int16_check_keystroke(memory),
            0x02 => self.int16_get_shift_flags(memory),
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
    fn int16_read_char<T: super::Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        // Check if there's already a character in the keyboard buffer
        let head_addr = BDA_START + BDA_KEYBOARD_BUFFER_HEAD;
        let tail_addr = BDA_START + BDA_KEYBOARD_BUFFER_TAIL;
        let head = memory.read_word(head_addr);
        let tail = memory.read_word(tail_addr);

        if head != tail {
            // Buffer has data - read from it
            let buffer_start = 0x001E; // Relative to BDA
            let char_addr = BDA_START + head as usize;
            let scan_code = memory.read_byte(char_addr);
            let ascii_code = memory.read_byte(char_addr + 1);

            // Update head pointer (circular buffer)
            let new_head = if head == buffer_start + 30 {
                buffer_start // Wrap around
            } else {
                head + 2
            };
            memory.write_word(head_addr, new_head);

            // Set return values
            self.ax = ((scan_code as u16) << 8) | (ascii_code as u16);
        } else {
            // Buffer is empty - read from I/O
            if let Some(key) = io.read_key() {
                self.ax = ((key.scan_code as u16) << 8) | (key.ascii_code as u16);
            } else {
                // No key available - return zero (this shouldn't happen in blocking mode)
                self.ax = 0;
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
    fn int16_check_keystroke(&mut self, memory: &Memory) {
        // Check keyboard buffer
        let head_addr = BDA_START + BDA_KEYBOARD_BUFFER_HEAD;
        let tail_addr = BDA_START + BDA_KEYBOARD_BUFFER_TAIL;
        let head = memory.read_word(head_addr);
        let tail = memory.read_word(tail_addr);

        if head != tail {
            // Buffer has data - peek at it without removing
            let char_addr = BDA_START + head as usize;
            let scan_code = memory.read_byte(char_addr);
            let ascii_code = memory.read_byte(char_addr + 1);

            // Set return values
            self.ax = ((scan_code as u16) << 8) | (ascii_code as u16);

            // Clear ZF to indicate keystroke available
            self.set_flag(cpu_flag::ZERO, false);
        } else {
            // No keystroke available - set ZF
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
    fn int16_get_shift_flags(&mut self, memory: &Memory) {
        let flags_addr = BDA_START + BDA_KEYBOARD_FLAGS1;
        let flags = memory.read_byte(flags_addr);

        // Return flags in AL
        self.ax = (self.ax & 0xFF00) | (flags as u16);
    }
}
