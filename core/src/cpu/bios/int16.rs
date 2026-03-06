use crate::Bus;
use crate::cpu::Cpu;
use crate::cpu::cpu_flag;
use crate::memory::{
    BDA_KEYBOARD_BUFFER_HEAD, BDA_KEYBOARD_BUFFER_TAIL, BDA_KEYBOARD_FLAGS1, BDA_START,
};

impl Cpu {
    pub(super) fn handle_int16(&mut self, bus: &mut Bus, io: &mut super::Bios) {
        match function {
            
            0x10 => self.int16_read_char(bus, io), // Extended read (same as 00h)
            0x11 => self.int16_check_keystroke(bus, io), // Extended check (same as 01h)
            0x12 => self.int16_get_shift_flags(bus), // Extended shift flags (same as 02h)
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






}