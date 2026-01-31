// INT 09h - Keyboard Hardware Interrupt Handler
//
// This is the hardware interrupt triggered when a key is pressed or released.
// In a real PC BIOS, this would:
// 1. Read scan code from keyboard controller (port 0x60)
// 2. Translate it and add to keyboard buffer
// 3. Send EOI to PIC
//
// In this emulator, the keyboard buffer is already populated by fire_keyboard_irq()
// before INT 09h is called, so this handler is intentionally minimal.
// It exists primarily so programs that install custom INT 09h handlers via the IVT
// have a default BIOS handler to chain to.

use super::Cpu;
use crate::memory::Memory;

impl Cpu {
    /// INT 09h - Keyboard Hardware Interrupt
    ///
    /// This is called when a keyboard IRQ fires. In this emulator, the keyboard
    /// buffer is already populated before this handler is invoked, so this is
    /// effectively a no-op that exists for compatibility with programs that
    /// install custom INT 09h handlers.
    pub(super) fn handle_int09(&mut self, _memory: &mut Memory) {
        // Keyboard buffer already populated by fire_keyboard_irq()
        // No additional work needed - just return (IRET will be handled by caller)
    }
}
