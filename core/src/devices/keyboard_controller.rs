use std::any::Any;

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
    obf: bool,

    // PS/2 auxiliary port (mouse)
    aux_buf: Vec<u8>,
    /// Read cursor into aux_buf.
    aux_read_pos: usize,
    /// Pending IRQ12 — set when bytes are pushed; cleared by take_pending_mouse().
    pending_mouse: bool,
    /// Auxiliary port enabled (8042 commands 0xA7/0xA8 on port 0x64).
    aux_enabled: bool,

    // A20 gate
    /// Pending multi-byte 8042 command (e.g. 0xD1 = write output port).
    pending_command: Option<u8>,
    /// Queued A20 gate change from 8042 output port write; drained by Bus.
    a20_request: Option<bool>,
    /// CPU reset requested (command 0xFE on port 0x64); drained by Bus.
    reset_request: bool,
}

impl KeyboardController {
    pub(crate) fn new() -> Self {
        Self {
            scan_code: 0,
            pending_key: false,
            obf: false,
            aux_buf: Vec::new(),
            aux_read_pos: 0,
            pending_mouse: false,
            aux_enabled: false,
            pending_command: None,
            a20_request: None,
            reset_request: false,
        }
    }

    /// Drain and return a pending A20 gate change requested via 8042 output port write.
    pub(crate) fn take_a20_request(&mut self) -> Option<bool> {
        self.a20_request.take()
    }

    /// Drain and return whether a CPU reset was requested (command 0xFE).
    pub(crate) fn take_reset_request(&mut self) -> bool {
        let r = self.reset_request;
        self.reset_request = false;
        r
    }

    pub(crate) fn key_press(&mut self, scan_code: u8) {
        self.scan_code = scan_code;
        self.pending_key = true;
        self.obf = true;
    }

    pub(crate) fn take_pending_key(&mut self) -> bool {
        let result = self.pending_key;
        self.pending_key = false;
        result
    }

    /// Returns true if a keyboard scan code is waiting at port 0x60.
    /// Used by Computer to gate queuing of the next key press.
    pub(crate) fn output_buffer_full(&self) -> bool {
        self.obf
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
        let pos = self.aux_read_pos;
        if pos > 0 {
            self.aux_buf.drain(..pos);
            self.aux_read_pos = 0;
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
        let pos = self.aux_read_pos;
        if pos < self.aux_buf.len() {
            let byte = self.aux_buf[pos];
            let new_pos = pos + 1;
            if new_pos == self.aux_buf.len() {
                self.aux_buf.clear();
                self.aux_read_pos = 0;
            } else {
                self.aux_read_pos = new_pos;
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
        self.aux_read_pos = 0;
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
        self.obf = false;
        self.aux_buf.clear();
        self.aux_read_pos = 0;
        self.pending_mouse = false;
        self.aux_enabled = false;
        self.pending_command = None;
        self.a20_request = None;
        self.reset_request = false;
    }

    fn memory_read_u8(&mut self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&mut self, port: u16, _cycle_count: u32) -> Option<u8> {
        match port {
            KEYBOARD_IO_PORT_DATA => {
                // On real 8042 hardware OBF and AUXOBF share a single output
                // buffer — a keyboard byte and an aux byte can never coexist.
                // We hold them in separate state, so we must pick one.
                // Keyboard OBF takes priority: if the keyboard side has data
                // (i.e. INT 09h was dispatched), return that and don't touch
                // the aux buffer.  This prevents INT 09h from consuming a
                // mouse-packet byte when both arrive close together.
                if self.obf {
                    self.obf = false;
                    Some(self.scan_code)
                } else {
                    let pos = self.aux_read_pos;
                    if pos < self.aux_buf.len() {
                        let byte = self.aux_buf[pos];
                        self.aux_read_pos = pos + 1;
                        Some(byte)
                    } else {
                        Some(self.scan_code)
                    }
                }
            }
            KEYBOARD_IO_PORT_STATUS => {
                let aux_has_data = self.aux_read_pos < self.aux_buf.len();
                let mut status = STATUS_SYSTEM;
                if self.obf || aux_has_data {
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
            KEYBOARD_IO_PORT_DATA => {
                // Port 0x60: data written after a pending 8042 command, or keyboard command.
                if self.pending_command == Some(0xD1) {
                    // 8042 write output port: bit 1 = A20 gate.
                    self.a20_request = Some((val & 0x02) != 0);
                    self.pending_command = None;
                    log::debug!(
                        "8042: output port write 0x{val:02X} → A20={}",
                        (val & 0x02) != 0
                    );
                }
                // Accept all port 0x60 writes (keyboard commands, LED writes, etc.)
                true
            }
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
                    0xD0 => {
                        // Read 8042 output port — response byte will be placed in output
                        // buffer. Bit 1 = A20 (always enabled in emulator).
                        // We use 0xCF: system reset high, A20 enabled, other bits set.
                        log::debug!("8042: read output port command (A20 always enabled)");
                    }
                    0xD1 => {
                        // Write 8042 output port — next byte written to port 0x60 is the value.
                        self.pending_command = Some(0xD1);
                        log::debug!("8042: write output port command pending");
                    }
                    0xFE => {
                        // Pulse reset line — triggers CPU reset
                        log::debug!("8042: CPU reset requested (command 0xFE)");
                        self.reset_request = true;
                    }
                    0xFF => {
                        // Keyboard controller self-test / reset — no-op (accept silently).
                        self.pending_command = None;
                    }
                    _ => log::warn!("8042: unhandled command 0x{val:02X} on port 0x64"),
                }
                true
            }
            _ => false,
        }
    }
}
