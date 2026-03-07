use std::{any::Any, cell::Cell};

use crate::Device;

pub const KEYBOARD_IO_PORT_DATA: u16 = 0x0060;
pub const KEYBOARD_IO_PORT_STATUS: u16 = 0x0064;
pub const KEYBOARD_IO_PORT_COMMAND: u16 = 0x0064;

/// Status register bit 0: Output Buffer Full — data ready at port 0x60
const STATUS_OBF: u8 = 0x01;
/// Status register bit 2: System flag — set after POST
const STATUS_SYSTEM: u8 = 0x04;
/// Status register bit 5: Auxiliary Output Buffer Full — data is from PS/2 mouse port
const STATUS_AUXOBF: u8 = 0x20;

/// 8042 PS/2 keyboard / auxiliary-port controller.
///
/// Handles both the keyboard (IRQ1 / INT 09h) and the PS/2 auxiliary mouse port
/// (IRQ12 / INT 74h) through the same IO ports 0x60 and 0x64.
pub(crate) struct KeyboardController {
    // Keyboard
    scan_code: u8,
    /// Set when a key scan code has been loaded; cleared by take_pending_key().
    pending_key: bool,
    /// Output Buffer Full (keyboard side) — cleared when port 0x60 is read.
    obf: Cell<bool>,

    // PS/2 auxiliary port (mouse)
    aux_buf: Vec<u8>,
    /// Read cursor into aux_buf; Cell<> so io_read_u8 (&self) can advance it.
    aux_read_pos: Cell<usize>,
    /// Pending IRQ12 — set when bytes are pushed; cleared by take_pending_mouse().
    pending_mouse: bool,
    /// Auxiliary port enabled (8042 commands 0xA7/0xA8 on port 0x64).
    aux_enabled: bool,
}

impl KeyboardController {
    pub(crate) fn new() -> Self {
        Self {
            scan_code: 0,
            pending_key: false,
            obf: Cell::new(false),
            aux_buf: Vec::new(),
            aux_read_pos: Cell::new(0),
            pending_mouse: false,
            aux_enabled: false,
        }
    }

    pub(crate) fn key_press(&mut self, scan_code: u8) {
        self.scan_code = scan_code;
        self.pending_key = true;
        self.obf.set(true);
    }

    pub(crate) fn take_pending_key(&mut self) -> bool {
        let result = self.pending_key;
        self.pending_key = false;
        result
    }

    /// Returns true if a keyboard scan code is waiting at port 0x60.
    /// Used by Computer to gate queuing of the next key press.
    pub(crate) fn output_buffer_full(&self) -> bool {
        self.obf.get()
    }

    // ── PS/2 auxiliary (mouse) port ──────────────────────────────────────────

    /// Enable or disable the PS/2 auxiliary port.
    pub(crate) fn set_aux_enabled(&mut self, enabled: bool) {
        self.aux_enabled = enabled;
        if !enabled {
            self.reset_aux();
        }
    }

    /// Queue raw PS/2 mouse packet bytes into the auxiliary output buffer.
    /// No-op when the auxiliary port is disabled.
    pub(crate) fn push_mouse_bytes(&mut self, bytes: &[u8]) {
        if !self.aux_enabled {
            return;
        }
        // Compact any already-consumed prefix before appending.
        let pos = self.aux_read_pos.get();
        if pos > 0 {
            self.aux_buf.drain(..pos);
            self.aux_read_pos.set(0);
        }
        self.aux_buf.extend_from_slice(bytes);
        self.pending_mouse = true;
    }

    /// Consume and return the IRQ12-pending flag.
    pub(crate) fn take_pending_mouse(&mut self) -> bool {
        let result = self.pending_mouse;
        self.pending_mouse = false;
        result
    }

    /// Read one byte from the auxiliary buffer (&mut path, used by INT 74h handler).
    pub(crate) fn aux_read(&mut self) -> Option<u8> {
        let pos = self.aux_read_pos.get();
        if pos < self.aux_buf.len() {
            let byte = self.aux_buf[pos];
            let new_pos = pos + 1;
            if new_pos == self.aux_buf.len() {
                self.aux_buf.clear();
                self.aux_read_pos.set(0);
            } else {
                self.aux_read_pos.set(new_pos);
            }
            Some(byte)
        } else {
            None
        }
    }

    #[cfg(test)]
    pub(crate) fn is_aux_enabled(&self) -> bool {
        self.aux_enabled
    }

    /// Reset the auxiliary port state (called on PS/2 mouse reset command).
    pub(crate) fn reset_aux(&mut self) {
        self.aux_buf.clear();
        self.aux_read_pos.set(0);
        self.pending_mouse = false;
    }
}

impl Device for KeyboardController {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.scan_code = 0;
        self.pending_key = false;
        self.obf.set(false);
        self.aux_buf.clear();
        self.aux_read_pos.set(0);
        self.pending_mouse = false;
        self.aux_enabled = false;
    }

    fn memory_read_u8(&self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16, _cycle_count: u32) -> Option<u8> {
        match port {
            KEYBOARD_IO_PORT_DATA => {
                // On real 8042 hardware OBF and AUXOBF share a single output
                // buffer — a keyboard byte and an aux byte can never coexist.
                // We hold them in separate state, so we must pick one.
                // Keyboard OBF takes priority: if the keyboard side has data
                // (i.e. INT 09h was dispatched), return that and don't touch
                // the aux buffer.  This prevents INT 09h from consuming a
                // mouse-packet byte when both arrive close together.
                if self.obf.get() {
                    self.obf.set(false);
                    Some(self.scan_code)
                } else {
                    let pos = self.aux_read_pos.get();
                    if pos < self.aux_buf.len() {
                        let byte = self.aux_buf[pos];
                        self.aux_read_pos.set(pos + 1);
                        Some(byte)
                    } else {
                        Some(self.scan_code)
                    }
                }
            }
            KEYBOARD_IO_PORT_STATUS => {
                let aux_has_data = self.aux_read_pos.get() < self.aux_buf.len();
                let mut status = STATUS_SYSTEM;
                if self.obf.get() || aux_has_data {
                    status |= STATUS_OBF;
                }
                if aux_has_data {
                    status |= STATUS_AUXOBF;
                }
                Some(status)
            }
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        match port {
            KEYBOARD_IO_PORT_COMMAND => {
                match val {
                    0xA7 => self.aux_enabled = false,
                    0xA8 => self.aux_enabled = true,
                    0xA9 => {
                        // Test auxiliary port — real hardware queues 0x00 (pass) via output
                        // buffer; our virtual BIOS handles this directly in INT 15h AH=C2h.
                    }
                    0xAD => {} // Disable keyboard interface — no-op
                    0xAE => {} // Enable keyboard interface — no-op
                    _ => log::warn!("8042: unhandled command 0x{val:02X} on port 0x64"),
                }
                true
            }
            _ => false,
        }
    }
}
