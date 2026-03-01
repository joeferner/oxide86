use std::{any::Any, collections::VecDeque};

use crate::{Device, KeyPress};

pub struct PIC {
    /// Queue of pending keyboard keys
    pending_keyboard_keys: VecDeque<KeyPress>,
}

impl PIC {
    pub fn new() -> Self {
        Self {
            pending_keyboard_keys: VecDeque::new(),
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
    pub fn push_keyboard_key(&mut self, key: KeyPress) {
        log::trace!(
            "Queueing keyboard IRQ: scan=0x{:02X}, ascii=0x{:02X}",
            key.scan_code,
            key.ascii_code
        );
        self.pending_keyboard_keys.push_back(key);
    }
}

impl Device for PIC {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn memory_read_u8(&self, _addr: usize) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn io_read_u8(&self, _port: u16) -> Option<u8> {
        None
    }

    fn io_write_u8(&mut self, _port: u16, _val: u8) -> bool {
        false
    }
}
