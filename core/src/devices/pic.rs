use std::{any::Any, cell::RefCell, rc::Rc};

use crate::{Device, devices::keyboard_controller::KeyboardController};

pub const PIC_IO_PORT_COMMAND: u16 = 0x0020;
pub const PIC_IO_PORT_MASK: u16 = 0x0021;

/// End of Interrupt
pub const PIC_COMMAND_EOI: u8 = 0x20;

pub const KEYBOARD_IRQ: u8 = 0x09;

/// IRQ line for the keyboard (IRQ1 → INT 9)
const KEYBOARD_IRQ_LINE: u8 = 1;

pub struct PIC {
    keyboard_controller: Rc<RefCell<KeyboardController>>,
    mask: u8,
    /// Bitmask of IRQ lines currently being serviced (awaiting EOI)
    in_service: u8,
}

impl PIC {
    pub fn new(keyboard_controller: Rc<RefCell<KeyboardController>>) -> Self {
        Self {
            keyboard_controller,
            mask: 0,
            in_service: 0,
        }
    }

    pub fn take_irq(&mut self) -> Option<u8> {
        let kbd_bit = 1u8 << KEYBOARD_IRQ_LINE;
        let masked = self.mask & kbd_bit != 0;
        let in_service = self.in_service & kbd_bit != 0;

        if !masked && !in_service && self.keyboard_controller.borrow_mut().take_pending_key() {
            self.in_service |= kbd_bit;
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

    fn reset(&mut self) {
        self.mask = 0;
        self.in_service = 0;
    }

    fn memory_read_u8(&self, _addr: usize) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16) -> Option<u8> {
        match port {
            PIC_IO_PORT_MASK => Some(self.mask),
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8) -> bool {
        match port {
            PIC_IO_PORT_COMMAND => {
                match val {
                    PIC_COMMAND_EOI => {
                        // Clear the highest-priority in-service bit (non-specific EOI)
                        if self.in_service != 0 {
                            let lowest_bit = self.in_service & self.in_service.wrapping_neg();
                            self.in_service &= !lowest_bit;
                        }
                    }
                    _ => log::warn!("unhandled PIC command 0x{val:02X}"),
                }
                true
            }
            PIC_IO_PORT_MASK => {
                self.mask = val;
                true
            }
            _ => false,
        }
    }
}
