use emu86_core::cpu::bios::KeyPress;
use std::io::{self, Read, Write};
use std::os::unix::io::AsRawFd;

// Console I/O operations for NativeBios

/// Check if stdin has data available (non-blocking)
fn stdin_has_data() -> bool {
    let stdin_fd = io::stdin().as_raw_fd();
    let mut pollfd = libc::pollfd {
        fd: stdin_fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // Poll with 0 timeout (immediate return)
    let result = unsafe { libc::poll(&mut pollfd, 1, 0) };
    result > 0 && (pollfd.revents & libc::POLLIN) != 0
}

/// Translate Unix line endings to DOS
fn translate_newline(ch: u8) -> u8 {
    if ch == 0x0A {
        0x0D // LF -> CR
    } else {
        ch
    }
}

pub fn read_char() -> Option<u8> {
    let mut buffer = [0u8; 1];
    match io::stdin().read_exact(&mut buffer) {
        Ok(_) => Some(translate_newline(buffer[0])),
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

/// Convert ASCII code to scan code
fn ascii_to_scan_code(ascii_code: u8) -> u8 {
    match ascii_code {
        0x0D => 0x1C, // Enter key
        0x08 => 0x0E, // Backspace
        0x1B => 0x01, // Escape
        _ => ascii_code,
    }
}

pub fn read_key() -> Option<KeyPress> {
    // Read a character from stdin (blocking)
    let mut buffer = [0u8; 1];
    match io::stdin().read_exact(&mut buffer) {
        Ok(_) => {
            let ascii_code = translate_newline(buffer[0]);
            let scan_code = ascii_to_scan_code(ascii_code);
            Some(KeyPress {
                scan_code,
                ascii_code,
            })
        }
        Err(_) => None,
    }
}

pub fn check_key() -> Option<KeyPress> {
    // Check if a key is available without blocking
    if !stdin_has_data() {
        return None;
    }

    // Data is available, read it
    let mut buffer = [0u8; 1];
    match io::stdin().read(&mut buffer) {
        Ok(1) => {
            let ascii_code = translate_newline(buffer[0]);
            let scan_code = ascii_to_scan_code(ascii_code);
            Some(KeyPress {
                scan_code,
                ascii_code,
            })
        }
        _ => None,
    }
}
