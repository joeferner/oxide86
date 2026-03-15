use std::{any::Any, cell::RefCell, rc::Rc};

use crate::{
    Device,
    devices::{keyboard_controller::KeyboardController, pit::Pit, uart::Uart},
};

// PIC1 (master) ports
pub const PIC_IO_PORT_COMMAND: u16 = 0x0020;
pub const PIC_IO_PORT_MASK: u16 = 0x0021;

// PIC2 (slave) ports
pub const PIC2_IO_PORT_COMMAND: u16 = 0x00A0;
pub const PIC2_IO_PORT_MASK: u16 = 0x00A1;

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

/// IRQ that the PS/2 mouse maps to (IRQ12 → INT 0x74, via PIC2)
pub const PS2_MOUSE_CPU_IRQ: u8 = 0x74;

/// IRQ line for the PIT (IRQ0, PIC1 bit 0)
const PIT_IRQ_LINE: u8 = 0;
/// IRQ line for the keyboard (IRQ1, PIC1 bit 1)
const KEYBOARD_IRQ_LINE: u8 = 1;
/// IRQ line for COM2/COM4 (IRQ3, PIC1 bit 3)
const COM2_IRQ_LINE: u8 = 3;
/// IRQ line for COM1/COM3 (IRQ4, PIC1 bit 4)
const COM1_IRQ_LINE: u8 = 4;

/// IRQ12 occupies bit 4 of PIC2 (PIC2 handles IRQ8–IRQ15, so IRQ12 = bit 4)
const PS2_MOUSE_PIC2_BIT: u8 = 4;

/// Non-PIT devices are only polled every this many take_irq() calls.
/// They do not require cycle-accurate timing; keyboard/UART/mouse latency
/// of ~100 instructions (~20 µs at 4.77 MHz) is imperceptible.
const NON_PIT_POLL_INTERVAL: u8 = 100;

pub(crate) struct Pic {
    pit: Rc<RefCell<Pit>>,
    keyboard_controller: Rc<RefCell<KeyboardController>>,
    uart: Rc<RefCell<Uart>>,

    // PIC1 (master) state
    mask: u8,
    /// Bitmask of PIC1 IRQ lines currently being serviced (awaiting EOI)
    in_service: u8,

    // PIC2 (slave) state — handles IRQ8–IRQ15 (PS/2 mouse is IRQ12)
    pic2_mask: u8,
    pic2_in_service: u8,

    /// Throttle counter for non-PIT IRQ polling. Incremented each take_irq()
    /// call; non-PIT devices only checked when it reaches NON_PIT_POLL_INTERVAL.
    non_pit_skip: u8,
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
            pic2_mask: 0,
            pic2_in_service: 0,
            non_pit_skip: 0,
        }
    }

    /// Signal that a non-PIT device has a pending IRQ, triggering an immediate
    /// check on the next take_irq() call rather than waiting up to
    /// NON_PIT_POLL_INTERVAL instructions.
    pub(crate) fn notify_pending(&mut self) {
        self.non_pit_skip = NON_PIT_POLL_INTERVAL - 1;
    }

    pub(crate) fn is_keyboard_irq_in_service(&self) -> bool {
        self.in_service & (1u8 << KEYBOARD_IRQ_LINE) != 0
    }

    /// Check and consume a pending timer (IRQ0) interrupt.
    ///
    /// Unlike `take_irq`, this only considers the PIT and does not consume any
    /// other pending IRQ. Used by the BIOS inline-dispatch path in `step()` to
    /// allow the timer to advance even when caller code runs with IF=0, without
    /// accidentally consuming keyboard or serial IRQs.
    pub(crate) fn take_timer_irq(&mut self, cycle_count: u32) -> bool {
        let bit = 1u8 << PIT_IRQ_LINE;
        if self.mask & bit != 0 {
            return false;
        }
        if self.in_service & bit != 0 {
            return false;
        }
        if !self.pit.borrow_mut().take_pending_timer_irq(cycle_count) {
            return false;
        }
        self.in_service |= bit;
        true
    }

    pub(crate) fn take_irq(&mut self, cycle_count: u32) -> Option<u8> {
        // PIT (IRQ0) — checked every instruction; timer accuracy is visible to software.
        {
            let bit = 1u8 << PIT_IRQ_LINE;
            let masked = self.mask & bit != 0;
            let in_service = self.in_service & bit != 0;

            if !masked && !in_service && self.pit.borrow_mut().take_pending_timer_irq(cycle_count) {
                self.in_service |= bit;
                return Some(PIT_CPU_IRQ);
            }
        }

        // Non-PIT devices (keyboard, UART, mouse) are polled every
        // NON_PIT_POLL_INTERVAL instructions. notify_pending() sets the
        // counter to NON_PIT_POLL_INTERVAL - 1 so the next call checks
        // immediately for keyboard/mouse events.
        self.non_pit_skip += 1;
        if self.non_pit_skip < NON_PIT_POLL_INTERVAL {
            return None;
        }
        // Prime the counter so any early return (IRQ delivered) causes a re-scan
        // on the very next instruction, draining all pending devices before backing off.
        // Only a full scan that finds nothing resets to 0 (see end of function).
        self.non_pit_skip = NON_PIT_POLL_INTERVAL - 1;

        // Keyboard (IRQ1)
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

        // PS/2 mouse — IRQ12 via PIC2 (cascaded through PIC1 IRQ2)
        {
            let pic2_bit = PS2_MOUSE_PIC2_BIT;
            let masked = self.pic2_mask & (1 << pic2_bit) != 0;
            let in_service = self.pic2_in_service & (1 << pic2_bit) != 0;

            if !masked && !in_service && self.keyboard_controller.borrow_mut().take_pending_mouse()
            {
                self.pic2_in_service |= 1 << pic2_bit;
                return Some(PS2_MOUSE_CPU_IRQ);
            }
        }

        // Full scan found nothing — back off to the normal polling interval.
        self.non_pit_skip = 0;
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
        self.pic2_mask = 0;
        self.pic2_in_service = 0;
        self.non_pit_skip = 0;
    }

    fn memory_read_u8(&mut self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&mut self, port: u16, _cycle_count: u32) -> Option<u8> {
        match port {
            PIC_IO_PORT_MASK => Some(self.mask),
            PIC2_IO_PORT_MASK => Some(self.pic2_mask),
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        match port {
            PIC_IO_PORT_COMMAND => {
                match val {
                    PIC_COMMAND_EOI => {
                        // Clear the highest-priority in-service bit (non-specific EOI)
                        if self.in_service != 0 {
                            let lowest_bit = self.in_service & self.in_service.wrapping_neg();
                            log::trace!(
                                "PIC: EOI clears in_service bit 0x{lowest_bit:02X} (was 0x{:02X})",
                                self.in_service
                            );
                            self.in_service &= !lowest_bit;
                        }
                    }
                    _ => log::warn!("unhandled PIC1 command 0x{val:02X}"),
                }
                true
            }
            PIC_IO_PORT_MASK => {
                log::debug!("PIC: mask set to 0x{val:02X}");
                self.mask = val;
                true
            }
            PIC2_IO_PORT_COMMAND => {
                match val {
                    PIC_COMMAND_EOI => {
                        if self.pic2_in_service != 0 {
                            let lowest_bit =
                                self.pic2_in_service & self.pic2_in_service.wrapping_neg();
                            self.pic2_in_service &= !lowest_bit;
                        }
                    }
                    _ => log::warn!("unhandled PIC2 command 0x{val:02X}"),
                }
                true
            }
            PIC2_IO_PORT_MASK => {
                self.pic2_mask = val;
                true
            }
            0x0028 => {
                log::debug!("PIC: write to port 0x0028 val=0x{val:02X} (unimplemented)");
                true
            }
            _ => false,
        }
    }
}
