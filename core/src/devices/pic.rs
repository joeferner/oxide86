use std::{any::Any, cell::RefCell, rc::Rc};

use crate::{Device, devices::keyboard_controller::KeyboardController};

pub const KEYBOARD_IRQ: u8 = 0x09;

pub struct PIC {
    keyboard_controller: Rc<RefCell<KeyboardController>>,
}

impl PIC {
    pub fn new(keyboard_controller: Rc<RefCell<KeyboardController>>) -> Self {
        Self {
            keyboard_controller,
        }
    }

    pub fn take_irq(&mut self) -> Option<u8> {
        // TODO this needs more logic on priorities etc
        if self.keyboard_controller.borrow_mut().take_pending_key() {
            Some(KEYBOARD_IRQ)
        } else {
            None
        }
    }
}

impl Device for PIC {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {}

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
