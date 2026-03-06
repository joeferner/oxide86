use std::{any::Any, cell::RefCell, rc::Rc};

use crate::{
    Device,
    devices::{keyboard_controller::KeyboardController, pit::Pit, uart::Uart},
};

pub const PIC_IO_PORT_COMMAND: u16 = 0x0020;
pub const PIC_IO_PORT_MASK: u16 = 0x0021;

/// End of Interrupt
pub const PIC_COMMAND_EOI: u8 = 0x20;

/// IRQ that PIT interrupts map to on the CPU
pub const PIT_CPU_IRQ: u8 = 0x08;
/// IRQ that keyboard interrupts map to on the CPU
pub const KEYBOARD_CPU_IRQ: u8 = 0x09;

/// IRQ that COM2/COM4 interrupts map to on the CPU (IRQ3 → INT 0x0B)
pub const COM2_CPU_IRQ: u8 = 0x0B;
/// IRQ that COM1/COM3 interrupts map to on the CPU (IRQ4 → INT 0x0C)
pub const COM1_CPU_IRQ: u8 = 0x0C;

/// IRQ line for the PIT (IRQ0)
const PIT_IRQ_LINE: u8 = 0;
/// IRQ line for the keyboard (IRQ1)
const KEYBOARD_IRQ_LINE: u8 = 1;
/// IRQ line for COM2/COM4 (IRQ3)
const COM2_IRQ_LINE: u8 = 3;
/// IRQ line for COM1/COM3 (IRQ4)
const COM1_IRQ_LINE: u8 = 4;

pub(crate) struct Pic {
    pit: Rc<RefCell<Pit>>,
    keyboard_controller: Rc<RefCell<KeyboardController>>,
    uart: Rc<RefCell<Uart>>,
    mask: u8,
    /// Bitmask of IRQ lines currently being serviced (awaiting EOI)
    in_service: u8,
}

impl Pic {
    pub(crate) fn new(
        pit: Rc<RefCell<Pit>>,
        keyboard_controller: Rc<RefCell<KeyboardController>>,
        uart: Rc<RefCell<Uart>>,
    ) -> Self {
        Self {
            pit,
            keyboard_controller,
            uart,
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

        // COM2/COM4 (IRQ3)
        {
            let bit = 1u8 << COM2_IRQ_LINE;
            let masked = self.mask & bit != 0;
            let in_service = self.in_service & bit != 0;

            if !masked && !in_service {
                let uart = self.uart.borrow();
                // bitwise OR to consume both without short-circuit
                if uart.take_pending_irq(1) | uart.take_pending_irq(3) {
                    self.in_service |= bit;
                    return Some(COM2_CPU_IRQ);
                }
            }
        }

        // COM1/COM3 (IRQ4)
        {
            let bit = 1u8 << COM1_IRQ_LINE;
            let masked = self.mask & bit != 0;
            let in_service = self.in_service & bit != 0;

            if !masked && !in_service {
                let uart = self.uart.borrow();
                // bitwise OR to consume both without short-circuit
                if uart.take_pending_irq(0) | uart.take_pending_irq(2) {
                    self.in_service |= bit;
                    return Some(COM1_CPU_IRQ);
                }
            }
        }

        None
    }
}

impl Device for Pic {
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
