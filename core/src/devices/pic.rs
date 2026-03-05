use std::{any::Any, cell::RefCell, rc::Rc};

use crate::{
    Device,
    devices::{keyboard_controller::KeyboardController, pit::PIT},
};

pub const PIC_IO_PORT_COMMAND: u16 = 0x0020;
pub const PIC_IO_PORT_MASK: u16 = 0x0021;

/// End of Interrupt
pub const PIC_COMMAND_EOI: u8 = 0x20;

/// IRQ that PIT interrupts map to on the CPU
pub const PIT_CPU_IRQ: u8 = 0x08;
/// IRQ that keyboard interrupts map to on the CPU
pub const KEYBOARD_CPU_IRQ: u8 = 0x09;

/// IRQ line for the PIT (IRQ1 → INT 9)
const PIT_IRQ_LINE: u8 = 0;
/// IRQ line for the keyboard (IRQ1 → INT 9)
const KEYBOARD_IRQ_LINE: u8 = 1;

pub struct PIC {
    pit: Rc<RefCell<PIT>>,
    keyboard_controller: Rc<RefCell<KeyboardController>>,
    mask: u8,
    /// Bitmask of IRQ lines currently being serviced (awaiting EOI)
    in_service: u8,
}

impl PIC {
    pub(crate) fn new(
        pit: Rc<RefCell<PIT>>,
        keyboard_controller: Rc<RefCell<KeyboardController>>,
    ) -> Self {
        Self {
            pit,
            keyboard_controller,
            mask: 0,
            in_service: 0,
        }
    }

    pub(crate) fn take_irq(&mut self, cycle_count: u32) -> Option<u8> {
        // pit
        {
            let bit = 1u8 << PIT_IRQ_LINE;
            let masked = self.mask & bit != 0;
            let in_service = self.in_service & bit != 0;

            if !masked && !in_service && self.pit.borrow_mut().take_pending_timer_irq(cycle_count) {
                self.in_service |= bit;
                return Some(PIT_CPU_IRQ);
            }
        }

        // keyboard
        {
            let bit = 1u8 << KEYBOARD_IRQ_LINE;
            let masked = self.mask & bit != 0;
            let in_service = self.in_service & bit != 0;

            if !masked && !in_service && self.keyboard_controller.borrow_mut().take_pending_key() {
                self.in_service |= bit;
                return Some(KEYBOARD_CPU_IRQ);
            }
        }

        None
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
