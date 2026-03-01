use std::{any::Any, cell::Cell};

use crate::{Device, KeyPress};

pub const KEYBOARD_IO_PORT_DATA: u16 = 0x0060;
pub const KEYBOARD_IO_PORT_STATUS: u16 = 0x0064;

/// Status register bit 0: Output Buffer Full — scan code ready to be read from port 0x60
const STATUS_OBF: u8 = 0x01;
/// Status register bit 2: System flag — set after POST to indicate normal operation
const STATUS_SYSTEM: u8 = 0x04;

pub struct KeyboardController {
    scan_code: u8,
    /// used by the PIC to check if a key has been pressed
    pending_key: bool,
    /// Output Buffer Full flag; uses Cell for interior mutability since io_read_u8 takes &self
    obf: Cell<bool>,
}

impl KeyboardController {
    pub fn new() -> Self {
        Self {
            scan_code: 0,
            pending_key: false,
            obf: Cell::new(false),
        }
    }

    pub fn push_key_press(&mut self, key: KeyPress) {
        self.scan_code = key.scan_code;
        self.pending_key = true;
        self.obf.set(true);
    }

    pub fn take_pending_key(&mut self) -> bool {
        let result = self.pending_key;
        self.pending_key = false;
        result
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
    }

    fn memory_read_u8(&self, _addr: usize) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16) -> Option<u8> {
        match port {
            KEYBOARD_IO_PORT_DATA => {
                self.obf.set(false);
                Some(self.scan_code)
            }
            KEYBOARD_IO_PORT_STATUS => {
                let mut status = STATUS_SYSTEM;
                if self.obf.get() {
                    status |= STATUS_OBF;
                }
                Some(status)
            }
            _ => None,
        }
    }

    fn io_write_u8(&mut self, _port: u16, _val: u8) -> bool {
        false
    }
}
