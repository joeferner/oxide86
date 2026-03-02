use crate::{
    bus::Bus,
    cpu::Cpu,
    cpu::bios::bda::bda_increment_timer_counter,
    devices::pic::{PIC_COMMAND_EOI, PIC_IO_PORT_COMMAND},
};

impl Cpu {
    /// INT 0x08 - Timer Hardware Interrupt (IRQ 0)
    ///
    /// Fired by the PIT channel 0 at approximately 18.2 Hz.  The handler
    /// increments the BDA timer tick counter and sends an End-Of-Interrupt
    /// to the PIC so subsequent timer IRQs can be delivered.
    pub(in crate::cpu) fn handle_int08_timer_interrupt(&mut self, bus: &mut Bus) {
        bda_increment_timer_counter(bus);
        bus.io_write_u8(PIC_IO_PORT_COMMAND, PIC_COMMAND_EOI);
    }
}
