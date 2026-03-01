use std::any::Any;

use crate::{Device, KeyPress};

pub const KEYBOARD_IO_PORT_DATA: u16 = 0x0060;

pub struct KeyboardController {
    scan_code: u8,
    pending_key: bool,
}

impl KeyboardController {
    pub fn new() -> Self {
        Self {
            scan_code: 0,
            pending_key: false,
        }
    }

    /// Queue a keyboard IRQ to be processed before the next instruction
    ///
    /// This method should be called from the event loop when a keyboard event is detected.
    /// The IRQ will be processed at the next opportunity (before the next instruction),
    /// which simulates the asynchronous nature of hardware interrupts.
    ///
    /// The INT 09h handler will:
    /// 1. Add the key to the BIOS keyboard buffer
    /// 2. Call any custom INT 09h handlers installed by the program
    ///
    /// Programs like edit.exe install custom INT 09h handlers to implement enhanced
    /// keyboard features and maintain their own keyboard buffers.
    pub fn push_key_press(&mut self, key: KeyPress) {
        log::trace!(
            "Queueing keyboard IRQ: scan=0x{:02X}, ascii=0x{:02X}",
            key.scan_code,
            key.ascii_code
        );
        self.scan_code = key.scan_code;
        self.pending_key = true;
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

    fn memory_read_u8(&self, _addr: usize) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16) -> Option<u8> {
        if port == KEYBOARD_IO_PORT_DATA {
            Some(self.scan_code)
        } else {
            None
        }
    }

    fn io_write_u8(&mut self, _port: u16, _val: u8) -> bool {
        false
    }
}
