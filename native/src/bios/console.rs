use emu86_core::cpu::bios::KeyPress;
use std::io::{self, Read, Write};

// Console I/O operations for NativeBios

pub fn read_char() -> Option<u8> {
    let mut buffer = [0u8; 1];
    match io::stdin().read_exact(&mut buffer) {
        Ok(_) => Some(buffer[0]),
        Err(_) => None,
    }
}

pub fn write_char(ch: u8) {
    print!("{}", ch as char);
    let _ = io::stdout().flush();
}

pub fn write_str(s: &str) {
    print!("{}", s);
    let _ = io::stdout().flush();
}

pub fn read_key() -> Option<KeyPress> {
    // Read a character from stdin
    let mut buffer = [0u8; 1];
    match io::stdin().read_exact(&mut buffer) {
        Ok(_) => {
            let ascii_code = buffer[0];
            // For simple implementation, use ASCII code as scan code
            // In a real implementation, we'd need to map special keys to proper scan codes
            let scan_code = match ascii_code {
                0x0D => 0x1C, // Enter key
                0x08 => 0x0E, // Backspace
                0x1B => 0x01, // Escape
                _ => ascii_code, // Use ASCII as scan code for regular keys
            };
            Some(KeyPress {
                scan_code,
                ascii_code,
            })
        }
        Err(_) => None,
    }
}
