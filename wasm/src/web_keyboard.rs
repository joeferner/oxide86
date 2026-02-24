//! Web-based keyboard input using browser keyboard events.

use oxide86_core::cpu::bios::KeyPress;
use oxide86_core::keyboard::KeyboardInput;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

/// Web-based keyboard input using browser keyboard events.
/// Event listeners are attached in JavaScript, which converts events to KeyPress via event_to_keypress.
pub struct WebKeyboard {
    /// Shared buffer for keyboard input from JavaScript events
    keyboard_buffer: Rc<RefCell<VecDeque<KeyPress>>>,
}

impl WebKeyboard {
    /// Create a new WebKeyboard.
    /// Event listeners should be attached in JavaScript code.
    pub fn new() -> Result<Self, JsValue> {
        Ok(Self {
            keyboard_buffer: Rc::new(RefCell::new(VecDeque::new())),
        })
    }
}

impl KeyboardInput for WebKeyboard {
    fn read_char(&mut self) -> Option<u8> {
        self.keyboard_buffer
            .borrow_mut()
            .pop_front()
            .map(|kp| kp.ascii_code)
    }

    fn check_char(&mut self) -> Option<u8> {
        self.keyboard_buffer
            .borrow()
            .front()
            .map(|kp| kp.ascii_code)
    }

    fn has_char_available(&self) -> bool {
        !self.keyboard_buffer.borrow().is_empty()
    }

    fn read_key(&mut self) -> Option<KeyPress> {
        self.keyboard_buffer.borrow_mut().pop_front()
    }

    fn check_key(&mut self) -> Option<KeyPress> {
        self.keyboard_buffer.borrow().front().copied()
    }
}

/// Convert JavaScript keyboard event data to 8086 KeyPress
pub(crate) fn event_to_keypress(
    code: &str,
    key: &str,
    shift: bool,
    ctrl: bool,
    alt: bool,
) -> Option<KeyPress> {
    // Handle Alt+key combinations (scan code with ASCII 0x00)
    if alt && !ctrl {
        return match code {
            "KeyA" => Some(KeyPress {
                scan_code: 0x1E,
                ascii_code: 0x00,
            }),
            "KeyB" => Some(KeyPress {
                scan_code: 0x30,
                ascii_code: 0x00,
            }),
            "KeyC" => Some(KeyPress {
                scan_code: 0x2E,
                ascii_code: 0x00,
            }),
            "KeyD" => Some(KeyPress {
                scan_code: 0x20,
                ascii_code: 0x00,
            }),
            "KeyE" => Some(KeyPress {
                scan_code: 0x12,
                ascii_code: 0x00,
            }),
            "KeyF" => Some(KeyPress {
                scan_code: 0x21,
                ascii_code: 0x00,
            }),
            "KeyG" => Some(KeyPress {
                scan_code: 0x22,
                ascii_code: 0x00,
            }),
            "KeyH" => Some(KeyPress {
                scan_code: 0x23,
                ascii_code: 0x00,
            }),
            "KeyI" => Some(KeyPress {
                scan_code: 0x17,
                ascii_code: 0x00,
            }),
            "KeyJ" => Some(KeyPress {
                scan_code: 0x24,
                ascii_code: 0x00,
            }),
            "KeyK" => Some(KeyPress {
                scan_code: 0x25,
                ascii_code: 0x00,
            }),
            "KeyL" => Some(KeyPress {
                scan_code: 0x26,
                ascii_code: 0x00,
            }),
            "KeyM" => Some(KeyPress {
                scan_code: 0x32,
                ascii_code: 0x00,
            }),
            "KeyN" => Some(KeyPress {
                scan_code: 0x31,
                ascii_code: 0x00,
            }),
            "KeyO" => Some(KeyPress {
                scan_code: 0x18,
                ascii_code: 0x00,
            }),
            "KeyP" => Some(KeyPress {
                scan_code: 0x19,
                ascii_code: 0x00,
            }),
            "KeyQ" => Some(KeyPress {
                scan_code: 0x10,
                ascii_code: 0x00,
            }),
            "KeyR" => Some(KeyPress {
                scan_code: 0x13,
                ascii_code: 0x00,
            }),
            "KeyS" => Some(KeyPress {
                scan_code: 0x1F,
                ascii_code: 0x00,
            }),
            "KeyT" => Some(KeyPress {
                scan_code: 0x14,
                ascii_code: 0x00,
            }),
            "KeyU" => Some(KeyPress {
                scan_code: 0x16,
                ascii_code: 0x00,
            }),
            "KeyV" => Some(KeyPress {
                scan_code: 0x2F,
                ascii_code: 0x00,
            }),
            "KeyW" => Some(KeyPress {
                scan_code: 0x11,
                ascii_code: 0x00,
            }),
            "KeyX" => Some(KeyPress {
                scan_code: 0x2D,
                ascii_code: 0x00,
            }),
            "KeyY" => Some(KeyPress {
                scan_code: 0x15,
                ascii_code: 0x00,
            }),
            "KeyZ" => Some(KeyPress {
                scan_code: 0x2C,
                ascii_code: 0x00,
            }),
            "Digit1" => Some(KeyPress {
                scan_code: 0x02,
                ascii_code: 0x00,
            }),
            "Digit2" => Some(KeyPress {
                scan_code: 0x03,
                ascii_code: 0x00,
            }),
            "Digit3" => Some(KeyPress {
                scan_code: 0x04,
                ascii_code: 0x00,
            }),
            "Digit4" => Some(KeyPress {
                scan_code: 0x05,
                ascii_code: 0x00,
            }),
            "Digit5" => Some(KeyPress {
                scan_code: 0x06,
                ascii_code: 0x00,
            }),
            "Digit6" => Some(KeyPress {
                scan_code: 0x07,
                ascii_code: 0x00,
            }),
            "Digit7" => Some(KeyPress {
                scan_code: 0x08,
                ascii_code: 0x00,
            }),
            "Digit8" => Some(KeyPress {
                scan_code: 0x09,
                ascii_code: 0x00,
            }),
            "Digit9" => Some(KeyPress {
                scan_code: 0x0A,
                ascii_code: 0x00,
            }),
            "Digit0" => Some(KeyPress {
                scan_code: 0x0B,
                ascii_code: 0x00,
            }),
            _ => None,
        };
    }

    // Handle Ctrl+key combinations
    if ctrl && !alt {
        return match code {
            "KeyA" => Some(KeyPress {
                scan_code: 0x1E,
                ascii_code: 0x01,
            }), // Ctrl+A
            "KeyB" => Some(KeyPress {
                scan_code: 0x30,
                ascii_code: 0x02,
            }), // Ctrl+B
            "KeyC" => Some(KeyPress {
                scan_code: 0x2E,
                ascii_code: 0x03,
            }), // Ctrl+C
            "KeyD" => Some(KeyPress {
                scan_code: 0x20,
                ascii_code: 0x04,
            }), // Ctrl+D
            "KeyE" => Some(KeyPress {
                scan_code: 0x12,
                ascii_code: 0x05,
            }), // Ctrl+E
            "KeyF" => Some(KeyPress {
                scan_code: 0x21,
                ascii_code: 0x06,
            }), // Ctrl+F
            "KeyG" => Some(KeyPress {
                scan_code: 0x22,
                ascii_code: 0x07,
            }), // Ctrl+G
            "KeyH" => Some(KeyPress {
                scan_code: 0x23,
                ascii_code: 0x08,
            }), // Ctrl+H (same as backspace)
            "KeyI" => Some(KeyPress {
                scan_code: 0x17,
                ascii_code: 0x09,
            }), // Ctrl+I (same as tab)
            "KeyJ" => Some(KeyPress {
                scan_code: 0x24,
                ascii_code: 0x0A,
            }), // Ctrl+J
            "KeyK" => Some(KeyPress {
                scan_code: 0x25,
                ascii_code: 0x0B,
            }), // Ctrl+K
            "KeyL" => Some(KeyPress {
                scan_code: 0x26,
                ascii_code: 0x0C,
            }), // Ctrl+L
            "KeyM" => Some(KeyPress {
                scan_code: 0x32,
                ascii_code: 0x0D,
            }), // Ctrl+M (same as enter)
            "KeyN" => Some(KeyPress {
                scan_code: 0x31,
                ascii_code: 0x0E,
            }), // Ctrl+N
            "KeyO" => Some(KeyPress {
                scan_code: 0x18,
                ascii_code: 0x0F,
            }), // Ctrl+O
            "KeyP" => Some(KeyPress {
                scan_code: 0x19,
                ascii_code: 0x10,
            }), // Ctrl+P
            "KeyQ" => Some(KeyPress {
                scan_code: 0x10,
                ascii_code: 0x11,
            }), // Ctrl+Q
            "KeyR" => Some(KeyPress {
                scan_code: 0x13,
                ascii_code: 0x12,
            }), // Ctrl+R
            "KeyS" => Some(KeyPress {
                scan_code: 0x1F,
                ascii_code: 0x13,
            }), // Ctrl+S
            "KeyT" => Some(KeyPress {
                scan_code: 0x14,
                ascii_code: 0x14,
            }), // Ctrl+T
            "KeyU" => Some(KeyPress {
                scan_code: 0x16,
                ascii_code: 0x15,
            }), // Ctrl+U
            "KeyV" => Some(KeyPress {
                scan_code: 0x2F,
                ascii_code: 0x16,
            }), // Ctrl+V
            "KeyW" => Some(KeyPress {
                scan_code: 0x11,
                ascii_code: 0x17,
            }), // Ctrl+W
            "KeyX" => Some(KeyPress {
                scan_code: 0x2D,
                ascii_code: 0x18,
            }), // Ctrl+X
            "KeyY" => Some(KeyPress {
                scan_code: 0x15,
                ascii_code: 0x19,
            }), // Ctrl+Y
            "KeyZ" => Some(KeyPress {
                scan_code: 0x2C,
                ascii_code: 0x1A,
            }), // Ctrl+Z
            _ => None,
        };
    }

    // Map common keys to scan codes and ASCII codes based on the code attribute
    let (scan_code, ascii_code) = match code {
        // Letter keys
        "KeyA" => (0x1E, if shift { b'A' } else { b'a' }),
        "KeyB" => (0x30, if shift { b'B' } else { b'b' }),
        "KeyC" => (0x2E, if shift { b'C' } else { b'c' }),
        "KeyD" => (0x20, if shift { b'D' } else { b'd' }),
        "KeyE" => (0x12, if shift { b'E' } else { b'e' }),
        "KeyF" => (0x21, if shift { b'F' } else { b'f' }),
        "KeyG" => (0x22, if shift { b'G' } else { b'g' }),
        "KeyH" => (0x23, if shift { b'H' } else { b'h' }),
        "KeyI" => (0x17, if shift { b'I' } else { b'i' }),
        "KeyJ" => (0x24, if shift { b'J' } else { b'j' }),
        "KeyK" => (0x25, if shift { b'K' } else { b'k' }),
        "KeyL" => (0x26, if shift { b'L' } else { b'l' }),
        "KeyM" => (0x32, if shift { b'M' } else { b'm' }),
        "KeyN" => (0x31, if shift { b'N' } else { b'n' }),
        "KeyO" => (0x18, if shift { b'O' } else { b'o' }),
        "KeyP" => (0x19, if shift { b'P' } else { b'p' }),
        "KeyQ" => (0x10, if shift { b'Q' } else { b'q' }),
        "KeyR" => (0x13, if shift { b'R' } else { b'r' }),
        "KeyS" => (0x1F, if shift { b'S' } else { b's' }),
        "KeyT" => (0x14, if shift { b'T' } else { b't' }),
        "KeyU" => (0x16, if shift { b'U' } else { b'u' }),
        "KeyV" => (0x2F, if shift { b'V' } else { b'v' }),
        "KeyW" => (0x11, if shift { b'W' } else { b'w' }),
        "KeyX" => (0x2D, if shift { b'X' } else { b'x' }),
        "KeyY" => (0x15, if shift { b'Y' } else { b'y' }),
        "KeyZ" => (0x2C, if shift { b'Z' } else { b'z' }),

        // Number keys (top row)
        "Digit1" => (0x02, if shift { b'!' } else { b'1' }),
        "Digit2" => (0x03, if shift { b'@' } else { b'2' }),
        "Digit3" => (0x04, if shift { b'#' } else { b'3' }),
        "Digit4" => (0x05, if shift { b'$' } else { b'4' }),
        "Digit5" => (0x06, if shift { b'%' } else { b'5' }),
        "Digit6" => (0x07, if shift { b'^' } else { b'6' }),
        "Digit7" => (0x08, if shift { b'&' } else { b'7' }),
        "Digit8" => (0x09, if shift { b'*' } else { b'8' }),
        "Digit9" => (0x0A, if shift { b'(' } else { b'9' }),
        "Digit0" => (0x0B, if shift { b')' } else { b'0' }),

        // Special characters
        "Minus" => (0x0C, if shift { b'_' } else { b'-' }),
        "Equal" => (0x0D, if shift { b'+' } else { b'=' }),
        "BracketLeft" => (0x1A, if shift { b'{' } else { b'[' }),
        "BracketRight" => (0x1B, if shift { b'}' } else { b']' }),
        "Backslash" => (0x2B, if shift { b'|' } else { b'\\' }),
        "Semicolon" => (0x27, if shift { b':' } else { b';' }),
        "Quote" => (0x28, if shift { b'"' } else { b'\'' }),
        "Comma" => (0x33, if shift { b'<' } else { b',' }),
        "Period" => (0x34, if shift { b'>' } else { b'.' }),
        "Slash" => (0x35, if shift { b'?' } else { b'/' }),
        "Backquote" => (0x29, if shift { b'~' } else { b'`' }),

        // Control keys
        "Enter" => (0x1C, 0x0D),
        "Space" => (0x39, b' '),
        "Escape" => (0x01, 0x1B),
        "Backspace" => (0x0E, 0x08),
        "Tab" => (0x0F, 0x09),

        // Arrow keys (extended keys - scan code 0x00 with special ASCII codes)
        "ArrowUp" => (0x48, 0x00),
        "ArrowDown" => (0x50, 0x00),
        "ArrowLeft" => (0x4B, 0x00),
        "ArrowRight" => (0x4D, 0x00),

        // Function keys
        "F1" => (0x3B, 0x00),
        "F2" => (0x3C, 0x00),
        "F3" => (0x3D, 0x00),
        "F4" => (0x3E, 0x00),
        "F5" => (0x3F, 0x00),
        "F6" => (0x40, 0x00),
        "F7" => (0x41, 0x00),
        "F8" => (0x42, 0x00),
        "F9" => (0x43, 0x00),
        "F10" => (0x44, 0x00),
        "F11" => (0x57, 0x00),
        "F12" => (0x58, 0x00),

        // Numpad keys - behavior depends on NumLock state.
        // The browser's KeyboardEvent.key already reflects NumLock:
        //   NumLock ON:  key = "0"-"9" or "."  → send digit ASCII
        //   NumLock OFF: key = navigation name  → send 0x00 (extended key)
        // Scan codes are the same regardless of NumLock (physical key position).
        "Numpad0" => (0x52, if key == "0" { b'0' } else { 0x00 }), // Insert when NumLock off
        "Numpad1" => (0x4F, if key == "1" { b'1' } else { 0x00 }), // End when NumLock off
        "Numpad2" => (0x50, if key == "2" { b'2' } else { 0x00 }), // Down when NumLock off
        "Numpad3" => (0x51, if key == "3" { b'3' } else { 0x00 }), // PgDn when NumLock off
        "Numpad4" => (0x4B, if key == "4" { b'4' } else { 0x00 }), // Left when NumLock off
        "Numpad5" => (0x4C, if key == "5" { b'5' } else { 0x00 }), // (undefined) when NumLock off
        "Numpad6" => (0x4D, if key == "6" { b'6' } else { 0x00 }), // Right when NumLock off
        "Numpad7" => (0x47, if key == "7" { b'7' } else { 0x00 }), // Home when NumLock off
        "Numpad8" => (0x48, if key == "8" { b'8' } else { 0x00 }), // Up when NumLock off
        "Numpad9" => (0x49, if key == "9" { b'9' } else { 0x00 }), // PgUp when NumLock off
        "NumpadMultiply" => (0x37, b'*'),
        "NumpadAdd" => (0x4E, b'+'),
        "NumpadSubtract" => (0x4A, b'-'),
        "NumpadDecimal" => (0x53, if key == "." { b'.' } else { 0x00 }), // Delete when NumLock off
        "NumpadDivide" => (0x35, b'/'),
        "NumpadEnter" => (0x1C, 0x0D),

        // Special keys
        "Insert" => (0x52, 0x00),
        "Delete" => (0x53, 0x00),
        "Home" => (0x47, 0x00),
        "End" => (0x4F, 0x00),
        "PageUp" => (0x49, 0x00),
        "PageDown" => (0x51, 0x00),

        // If we can't map the code, try to extract ASCII from the key string
        _ => {
            if key.len() == 1 {
                let ch = key.chars().next()?;
                if ch.is_ascii() {
                    (0x00, ch as u8) // Generic scan code, use ASCII
                } else {
                    return None;
                }
            } else {
                log::warn!("Unmapped keyboard code: {}", code);
                return None;
            }
        }
    };

    Some(KeyPress {
        scan_code,
        ascii_code,
    })
}
