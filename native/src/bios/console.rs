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

/// Poll stdin with a timeout in milliseconds
fn stdin_poll_timeout(timeout_ms: i32) -> bool {
    let stdin_fd = io::stdin().as_raw_fd();
    let mut pollfd = libc::pollfd {
        fd: stdin_fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let result = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
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

/// Check if a character is available and return it (non-blocking)
/// Used by INT 21h, AH=06h (Direct Console I/O)
pub fn check_char() -> Option<u8> {
    if !stdin_has_data() {
        return None;
    }

    let mut buffer = [0u8; 1];
    match io::stdin().read(&mut buffer) {
        Ok(1) => Some(translate_newline(buffer[0])),
        _ => None,
    }
}

/// Check if a character is available without consuming it
/// Used by INT 21h, AH=0Bh (Check Input Status)
pub fn has_char_available() -> bool {
    stdin_has_data()
}

pub fn write_char(ch: u8) {
    print!("{}", ch as char);
    let _ = io::stdout().flush();
}

pub fn write_str(s: &str) {
    print!("{}", s);
    let _ = io::stdout().flush();
}

/// Convert ASCII code to scan code for regular keys
fn ascii_to_scan_code(ascii_code: u8) -> u8 {
    match ascii_code {
        0x0D => 0x1C, // Enter key
        0x08 => 0x0E, // Backspace
        0x1B => 0x01, // Escape
        0x09 => 0x0F, // Tab
        0x7F => 0x0E, // DEL -> Backspace
        _ => 0x00,    // Unknown - let caller handle
    }
}

/// Try to read an escape sequence and return the corresponding KeyPress
/// Returns None if not a recognized escape sequence
fn try_read_escape_sequence() -> Option<KeyPress> {
    // We've already read ESC (0x1B). Check if more data follows quickly.
    // If no data within a short timeout, it's just the ESC key.
    // Use 200ms timeout to handle slow terminal escape sequences
    if !stdin_poll_timeout(200) {
        // Just ESC key
        return Some(KeyPress {
            scan_code: 0x01,
            ascii_code: 0x1B,
        });
    }

    let mut buffer = [0u8; 1];
    if io::stdin().read_exact(&mut buffer).is_err() {
        return Some(KeyPress {
            scan_code: 0x01,
            ascii_code: 0x1B,
        });
    }

    match buffer[0] {
        b'[' => {
            // CSI sequence: ESC [ ...
            parse_csi_sequence()
        }
        b'O' => {
            // SS3 sequence: ESC O ... (used by some terminals for F1-F4)
            parse_ss3_sequence()
        }
        _ => {
            // Alt+key combination or unknown sequence
            // Return ESC and the key will be read next time
            Some(KeyPress {
                scan_code: 0x01,
                ascii_code: 0x1B,
            })
        }
    }
}

/// Parse CSI sequence (ESC [ ...)
fn parse_csi_sequence() -> Option<KeyPress> {
    let mut buffer = [0u8; 1];
    if io::stdin().read_exact(&mut buffer).is_err() {
        return None;
    }

    match buffer[0] {
        b'A' => Some(KeyPress {
            scan_code: 0x48,
            ascii_code: 0x00,
        }), // Up arrow
        b'B' => Some(KeyPress {
            scan_code: 0x50,
            ascii_code: 0x00,
        }), // Down arrow
        b'C' => Some(KeyPress {
            scan_code: 0x4D,
            ascii_code: 0x00,
        }), // Right arrow
        b'D' => Some(KeyPress {
            scan_code: 0x4B,
            ascii_code: 0x00,
        }), // Left arrow
        b'H' => Some(KeyPress {
            scan_code: 0x47,
            ascii_code: 0x00,
        }), // Home
        b'F' => Some(KeyPress {
            scan_code: 0x4F,
            ascii_code: 0x00,
        }), // End
        b'1' | b'2' | b'3' | b'4' | b'5' | b'6' => {
            // Extended sequence like ESC [ 1 1 ~ for F1
            parse_extended_csi_sequence(buffer[0])
        }
        _ => None,
    }
}

/// Parse extended CSI sequence (ESC [ digit digit ~ for function keys)
fn parse_extended_csi_sequence(first_digit: u8) -> Option<KeyPress> {
    let mut buffer = [0u8; 1];

    // Read next character
    if io::stdin().read_exact(&mut buffer).is_err() {
        return None;
    }

    let second_char = buffer[0];

    // Handle single-digit codes like ESC [ 2 ~ (Insert)
    if second_char == b'~' {
        return match first_digit {
            b'2' => Some(KeyPress {
                scan_code: 0x52,
                ascii_code: 0x00,
            }), // Insert
            b'3' => Some(KeyPress {
                scan_code: 0x53,
                ascii_code: 0x00,
            }), // Delete
            b'5' => Some(KeyPress {
                scan_code: 0x49,
                ascii_code: 0x00,
            }), // Page Up
            b'6' => Some(KeyPress {
                scan_code: 0x51,
                ascii_code: 0x00,
            }), // Page Down
            _ => None,
        };
    }

    // Two-digit code, read the tilde
    let mut tilde = [0u8; 1];
    if io::stdin().read_exact(&mut tilde).is_err() || tilde[0] != b'~' {
        return None;
    }

    // Parse two-digit function key codes
    let code = (first_digit - b'0') * 10 + (second_char - b'0');

    match code {
        11 => Some(KeyPress {
            scan_code: 0x3B,
            ascii_code: 0x00,
        }), // F1
        12 => Some(KeyPress {
            scan_code: 0x3C,
            ascii_code: 0x00,
        }), // F2
        13 => Some(KeyPress {
            scan_code: 0x3D,
            ascii_code: 0x00,
        }), // F3
        14 => Some(KeyPress {
            scan_code: 0x3E,
            ascii_code: 0x00,
        }), // F4
        15 => Some(KeyPress {
            scan_code: 0x3F,
            ascii_code: 0x00,
        }), // F5
        17 => Some(KeyPress {
            scan_code: 0x40,
            ascii_code: 0x00,
        }), // F6
        18 => Some(KeyPress {
            scan_code: 0x41,
            ascii_code: 0x00,
        }), // F7
        19 => Some(KeyPress {
            scan_code: 0x42,
            ascii_code: 0x00,
        }), // F8
        20 => Some(KeyPress {
            scan_code: 0x43,
            ascii_code: 0x00,
        }), // F9
        21 => Some(KeyPress {
            scan_code: 0x44,
            ascii_code: 0x00,
        }), // F10
        23 => Some(KeyPress {
            scan_code: 0x85,
            ascii_code: 0x00,
        }), // F11
        24 => Some(KeyPress {
            scan_code: 0x86,
            ascii_code: 0x00,
        }), // F12
        _ => None,
    }
}

/// Parse SS3 sequence (ESC O ... for F1-F4 on some terminals)
fn parse_ss3_sequence() -> Option<KeyPress> {
    let mut buffer = [0u8; 1];
    if io::stdin().read_exact(&mut buffer).is_err() {
        return None;
    }

    match buffer[0] {
        b'P' => Some(KeyPress {
            scan_code: 0x3B,
            ascii_code: 0x00,
        }), // F1
        b'Q' => Some(KeyPress {
            scan_code: 0x3C,
            ascii_code: 0x00,
        }), // F2
        b'R' => Some(KeyPress {
            scan_code: 0x3D,
            ascii_code: 0x00,
        }), // F3
        b'S' => Some(KeyPress {
            scan_code: 0x3E,
            ascii_code: 0x00,
        }), // F4
        b'H' => Some(KeyPress {
            scan_code: 0x47,
            ascii_code: 0x00,
        }), // Home (alternate)
        b'F' => Some(KeyPress {
            scan_code: 0x4F,
            ascii_code: 0x00,
        }), // End (alternate)
        _ => None,
    }
}

pub fn read_key() -> Option<KeyPress> {
    let mut buffer = [0u8; 1];
    match io::stdin().read_exact(&mut buffer) {
        Ok(_) => {
            let ch = buffer[0];
            log::info!("read_key.... 0x{:02x}", ch);

            // Check for escape sequence
            if ch == 0x1B {
                return try_read_escape_sequence();
            }

            let ascii_code = translate_newline(ch);
            let scan_code = ascii_to_scan_code(ascii_code);

            // For printable ASCII characters, derive scan code from ASCII
            let final_scan_code = if scan_code == 0x00 && (0x20..0x7F).contains(&ascii_code) {
                // Use a simple mapping for printable chars
                ascii_code
            } else {
                scan_code
            };

            Some(KeyPress {
                scan_code: final_scan_code,
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
            let ch = buffer[0];
            log::info!("check_key.... 0x{:02x}", ch);

            // Check for escape sequence
            if ch == 0x1B {
                return try_read_escape_sequence();
            }

            let ascii_code = translate_newline(ch);
            let scan_code = ascii_to_scan_code(ascii_code);

            // For printable ASCII characters, derive scan code from ASCII
            let final_scan_code = if scan_code == 0x00 && (0x20..0x7F).contains(&ascii_code) {
                ascii_code
            } else {
                scan_code
            };

            Some(KeyPress {
                scan_code: final_scan_code,
                ascii_code,
            })
        }
        _ => None,
    }
}
