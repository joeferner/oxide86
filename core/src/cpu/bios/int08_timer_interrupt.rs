use crate::{
    bus::Bus,
    cpu::bios::bda::bda_increment_timer_counter,
    cpu::{Cpu, bios::BIOS_CODE_SEGMENT, cpu_flag},
    devices::pic::{PIC_COMMAND_EOI, PIC_IO_PORT_COMMAND},
};

/// IP within the BIOS code segment used as the IRET trampoline after the
/// INT 1Ch user timer tick handler returns.  Must be ≤ 0xFF and must not
/// collide with any real INT vector handled in step_bios_int.
pub(in crate::cpu) const INT1C_RETURN_IP: u16 = 0xF5;

/// IP within the BIOS code segment used as the IRET trampoline when a timer
/// IRQ is inline-dispatched to a guest INT 08h handler (i.e. when a BIOS
/// handler sets IF=1 and the timer fires, but IVT[0x08] points to guest code).
/// After the guest handler IRETs here, patch_flags_and_iret finishes the
/// original BIOS call's IRET to its caller.  Must not collide with any real
/// INT vector handled in step_bios_int.
pub(in crate::cpu) const TIMER_INLINE_RETURN_IP: u16 = 0xF6;

impl Cpu {
    /// INT 0x08 - Timer Hardware Interrupt (IRQ 0)
    ///
    /// Fired by the PIT channel 0 at approximately 18.2 Hz.  The handler
    /// increments the BDA timer tick counter, sends an End-Of-Interrupt to
    /// the PIC, then chains to INT 1Ch (user timer tick) exactly as the real
    /// IBM BIOS does — by executing a software INT 1Ch from within the handler.
    ///
    /// Programs that hook INT 1Ch (music drivers, animation timers, etc.) rely
    /// on this chain.  We simulate the INT 1Ch instruction by pushing an IRET
    /// frame targeting our trampoline and jumping to IVT[1Ch].  When the
    /// handler IRETs to the trampoline, step() calls step_bios_int(INT1C_RETURN_IP)
    /// (a no-op) and patch_flags_and_iret then IRETs to the original caller.
    pub(in crate::cpu) fn handle_int08_timer_interrupt(&mut self, bus: &mut Bus) {
        bus.increment_cycle_count(150);
        bda_increment_timer_counter(bus);
        bus.io_write_u8(PIC_IO_PORT_COMMAND, PIC_COMMAND_EOI);

        // Chain to INT 1Ch — equivalent to the ROM executing `INT 1Ch`.
        let ivt_1c = 0x1C_usize * 4;
        let handler_ip = bus.memory_read_u16(ivt_1c);
        let handler_cs = bus.memory_read_u16(ivt_1c + 2);
        if handler_cs != BIOS_CODE_SEGMENT {
            self.push(self.flags, bus);
            self.push(BIOS_CODE_SEGMENT, bus);
            self.push(INT1C_RETURN_IP, bus);
            self.set_flag(cpu_flag::INTERRUPT, false);
            self.cs = handler_cs;
            self.ip = handler_ip;
            // step() sees CS != BIOS_CODE_SEGMENT and skips patch_flags_and_iret,
            // letting the handler run as normal x86 code.
        }
    }
}
