use crate::Bus;
use crate::cpu::Cpu;

impl Cpu {
    /// INT 0x1C - User Timer Tick Hook
    ///
    /// This interrupt is called by the INT 0x08 handler (system timer) on every
    /// timer tick (18.2 times per second). By default, it does nothing and just
    /// returns immediately.
    ///
    /// Programs can install custom INT 0x1C handlers via the IVT to perform
    /// periodic tasks such as:
    /// - Music playback (QBASIC PLAY command)
    /// - Animation updates
    /// - Background polling
    /// - TSR (Terminate and Stay Resident) program operations
    ///
    /// The default BIOS handler is a no-op (just IRET).
    #[allow(unused_variables)]
    pub(super) fn handle_int1c(&mut self, bus: &mut Bus, io: &mut super::Bios) {
        // Default handler does nothing - just returns
        // Programs that need periodic callbacks will install their own handler
        // by modifying the IVT entry for INT 0x1C
    }
}
