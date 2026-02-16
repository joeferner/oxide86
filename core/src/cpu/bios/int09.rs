// INT 09h - Keyboard Hardware Interrupt Handler
//
// This is the hardware interrupt triggered when a key is pressed or released.
// In a real PC BIOS, this handler would:
// 1. Read scan code from keyboard controller (port 0x60)
// 2. Translate it to ASCII and add to keyboard buffer
// 3. Send EOI to PIC
//
// IBM AT BIOS behavior: Before buffering a key, calls INT 15h AH=4Fh (keyboard intercept).
// Task managers (like FDC8) install INT 15h handlers to route keystrokes to their
// own buffers. If INT 15h returns CF=1, the key has been handled and BIOS skips buffering.
//
// The INT 15h AH=4Fh call is handled asynchronously by the F000 return path in
// computer.rs when a custom INT 15h handler is installed. This handler only does
// the direct buffering case (used when called directly via fire_keyboard_irq or
// execute_int_with_io).
//
// This emulator's default BIOS handler reads the scan code and ASCII from the BIOS
// struct and adds them to the BIOS keyboard buffer. Programs that install custom INT 09h
// handlers can read port 0x60 directly and handle keys themselves.

use super::{Bios, Cpu};
use crate::memory::{BDA_KEYBOARD_BUFFER_HEAD, BDA_KEYBOARD_BUFFER_TAIL, BDA_START, Memory};

impl Cpu {
    /// INT 09h - Keyboard Hardware Interrupt
    ///
    /// This is the default BIOS handler that reads keyboard data and adds it to the buffer.
    /// Programs with custom INT 09h handlers will replace this via the IVT and handle
    /// keyboard input directly by reading port 0x60.
    ///
    /// Note: when called via the F000 CALL FAR path (computer.rs step()), the caller
    /// is responsible for invoking INT 15h AH=4Fh first if a custom INT 15h is installed.
    /// This handler only buffers; it does not call INT 15h itself.
    pub(super) fn handle_int09(&mut self, memory: &mut Memory, io: &mut Bios) {
        // Read keyboard data from BIOS struct (set by fire_keyboard_irq)
        let scan_code = io.pending_scan_code;
        let ascii_code = io.pending_ascii_code;

        // Check if this is a key release (bit 7 set)
        // Key releases should NOT be added to the BIOS buffer - they're only for custom handlers
        if scan_code & 0x80 != 0 {
            log::debug!(
                "INT 09h (BIOS): Key release detected (scan=0x{:02X}), not buffering",
                scan_code
            );
            return;
        }

        // Check if this key was already pre-buffered by a custom handler
        // If so, skip adding it again (prevents duplicates when custom handlers chain to us)
        if io.key_was_prebuffered {
            log::debug!(
                "INT 09h (BIOS): Key already pre-buffered (chained from custom handler), skipping"
            );
            io.key_was_prebuffered = false; // Reset flag
            return;
        }

        Self::add_key_to_bda_buffer(memory, scan_code, ascii_code);
    }

    /// Add a key press to the BIOS Data Area keyboard ring buffer.
    /// Called by handle_int09 and by the F000:0xFF continuation after INT 15h returns CF=0.
    pub(super) fn add_key_to_bda_buffer(memory: &mut Memory, scan_code: u8, ascii_code: u8) {
        let head_addr = BDA_START + BDA_KEYBOARD_BUFFER_HEAD;
        let tail_addr = BDA_START + BDA_KEYBOARD_BUFFER_TAIL;
        let head = memory.read_u16(head_addr);
        let tail = memory.read_u16(tail_addr);

        // Calculate what tail would be after adding this key
        let buffer_start: u16 = 0x001E; // Relative to BDA
        let new_tail = if tail == buffer_start + 30 {
            buffer_start // Wrap around
        } else {
            tail + 2
        };

        // Check if buffer would become full
        if new_tail == head {
            log::warn!(
                "INT 09h (BIOS): Keyboard buffer full! Discarding scan=0x{:02X}, ascii=0x{:02X}",
                scan_code,
                ascii_code
            );
            return;
        }

        // Add key to buffer
        let char_addr = BDA_START + tail as usize;
        memory.write_u8(char_addr, scan_code);
        memory.write_u8(char_addr + 1, ascii_code);
        memory.write_u16(tail_addr, new_tail);

        log::debug!(
            "INT 09h (BIOS): Added to buffer - Scan: 0x{:02X}, ASCII: 0x{:02X} ('{}')",
            scan_code,
            ascii_code,
            if (0x20..0x7F).contains(&ascii_code) {
                ascii_code as char
            } else {
                '.'
            }
        );
    }
}
