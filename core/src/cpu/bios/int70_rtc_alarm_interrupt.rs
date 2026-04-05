use crate::{
    bus::Bus,
    cpu::{Cpu, bios::BIOS_CODE_SEGMENT, cpu_flag},
    devices::{
        pic::{PIC_COMMAND_EOI, PIC_IO_PORT_COMMAND, PIC2_IO_PORT_COMMAND},
        rtc::{RTC_IO_PORT_DATA, RTC_IO_PORT_REGISTER_SELECT},
    },
};

/// IP within the BIOS code segment used as the IRET trampoline after the
/// INT 4Ah user alarm handler returns.  Must be ≤ 0xFF and must not collide
/// with any real INT vector handled in step_bios_int.
pub(in crate::cpu) const INT4A_RETURN_IP: u16 = 0xF3;

impl Cpu {
    /// INT 70h — RTC alarm hardware interrupt (IRQ8).
    ///
    /// Acknowledges the interrupt by reading Status Register C (clearing AF/IRQF),
    /// sends EOI to PIC2 and PIC1 so IRQ8 can fire again, then chains to INT 4Ah
    /// (user alarm interrupt) if the Alarm Flag was set — exactly as the IBM AT BIOS does.
    /// Programs that hook INT 4Ah (e.g. CheckIt) rely on this chain to receive alarm
    /// notifications.
    pub(in crate::cpu) fn handle_int70_rtc_alarm_interrupt(&mut self, bus: &mut Bus) {
        bus.increment_cycle_count(200);

        // Read Status Register C (0x0C) to acknowledge and clear AF/IRQF flags.
        bus.io_write_u8(RTC_IO_PORT_REGISTER_SELECT, 0x0C);
        let status_c = bus.io_read_u8(RTC_IO_PORT_DATA);

        // Acknowledge both PICs so IRQ8 can fire again.
        bus.io_write_u8(PIC2_IO_PORT_COMMAND, PIC_COMMAND_EOI);
        bus.io_write_u8(PIC_IO_PORT_COMMAND, PIC_COMMAND_EOI);

        // If the Alarm Flag (bit 5) was set, chain to INT 4Ah (user alarm interrupt).
        if status_c & 0x20 != 0 {
            let ivt_4a = 0x4A_usize * 4;
            let handler_ip = bus.memory_read_u16(ivt_4a);
            let handler_cs = bus.memory_read_u16(ivt_4a + 2);
            if handler_cs != BIOS_CODE_SEGMENT {
                self.push(self.flags, bus);
                self.push(BIOS_CODE_SEGMENT, bus);
                self.push(INT4A_RETURN_IP, bus);
                self.set_flag(cpu_flag::INTERRUPT, false);
                self.set_cs_real(handler_cs);
                self.ip = handler_ip;
            }
        }
    }
}
