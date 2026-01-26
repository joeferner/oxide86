use crate::cpu::Cpu;

impl Cpu {
    /// INT 0x35-0x3F - Reserved for DOS
    /// These are placeholder interrupts that simply return.
    /// DOS reserves INT 35h-3Fh for internal use, but they are typically unused
    /// and just return immediately (IRET).
    pub(super) fn handle_int35_3f(&mut self, int_num: u8) {
        log::trace!("INT {:02X}h: Reserved interrupt (no-op)", int_num);
        // No operation - just return
    }
}
