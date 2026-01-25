/// BIOS and DOS interrupt handler trait and implementation
/// The core provides the interrupt dispatch mechanism, but I/O is handled via callbacks

use super::Cpu;
use crate::memory::Memory;
use log::warn;

/// Trait for handling BIOS interrupt I/O operations
/// Platform-specific code (native, WASM) implements this to provide actual I/O
pub trait Bios {
    /// Read a character from standard input
    fn read_char(&mut self) -> Option<u8>;

    /// Write a character to standard output
    fn write_char(&mut self, ch: u8);

    /// Write a string to standard output
    fn write_str(&mut self, s: &str);
}

/// A null I/O handler that does nothing (for testing or headless operation)
pub struct NullBios;

impl Bios for NullBios {
    fn read_char(&mut self) -> Option<u8> {
        None
    }

    fn write_char(&mut self, _ch: u8) {
        // Do nothing
    }

    fn write_str(&mut self, _s: &str) {
        // Do nothing
    }
}

impl Cpu {
    /// Handle BIOS/DOS interrupts with provided I/O handler
    /// Returns true if the interrupt was handled, false if it should proceed normally
    pub(super) fn handle_bios_interrupt<T: Bios>(
        &mut self,
        int_num: u8,
        memory: &mut Memory,
        io: &mut T,
    ) -> bool {
        match int_num {
            0x21 => {
                self.handle_int21(memory, io);
                true
            }
            // Other BIOS interrupts can be added here
            // 0x10 => Video services
            // 0x13 => Disk services
            // 0x16 => Keyboard services
            // etc.
            _ => {
                warn!("Unhandled BIOS interrupt: 0x{:02X}", int_num);
                false // Not handled, proceed with normal interrupt mechanism
            }
        }
    }

    /// INT 0x21 - DOS Services
    /// AH register contains the function number
    fn handle_int21<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let function = (self.ax >> 8) as u8; // Get AH directly

        match function {
            0x01 => self.int21_read_char_with_echo(io),
            0x02 => self.int21_write_char(io),
            0x09 => self.int21_write_string(memory, io),
            0x4C => self.int21_exit(),
            _ => {
                warn!("Unhandled INT 0x21 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 21h, AH=01h - Read Character from STDIN with Echo
    /// Returns: AL = character read
    fn int21_read_char_with_echo<T: Bios>(&mut self, io: &mut T) {
        if let Some(ch) = io.read_char() {
            // Echo the character
            io.write_char(ch);
            // Store in AL
            self.ax = (self.ax & 0xFF00) | (ch as u16);
        }
    }

    /// INT 21h, AH=02h - Write Character to STDOUT
    /// Input: DL = character to write
    fn int21_write_char<T: Bios>(&mut self, io: &mut T) {
        let ch = self.get_reg8(2); // DL register
        io.write_char(ch);
    }

    /// INT 21h, AH=09h - Write String to STDOUT
    /// Input: DS:DX = pointer to '$'-terminated string
    fn int21_write_string<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let mut addr = Self::physical_address(self.ds, self.dx);
        let mut output = String::new();

        loop {
            let ch = memory.read_byte(addr);
            if ch == b'$' {
                break;
            }
            output.push(ch as char);
            addr += 1;
        }

        io.write_str(&output);
    }

    /// INT 21h, AH=4Ch - Exit Program
    /// Input: AL = return code
    fn int21_exit(&mut self) {
        // Halt the CPU
        self.halted = true;
    }
}
