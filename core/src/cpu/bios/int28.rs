// INT 28h - DOS Idle Interrupt
// Called by DOS when waiting for keyboard input (in the idle loop).
// TSR (Terminate and Stay Resident) programs hook this interrupt
// to perform background processing when DOS is idle.

use super::Cpu;

impl Cpu {
    /// INT 28h - DOS Idle Interrupt
    ///
    /// This interrupt is called repeatedly by DOS while waiting for keyboard
    /// input. It provides a hook for TSR programs to safely call certain DOS
    /// functions (those that don't use the DOS critical error handler stack).
    ///
    /// Our implementation is a no-op since we don't have TSRs to signal.
    /// Programs that hook this interrupt will get control when it's called.
    pub(super) fn handle_int28(&mut self) {
        // No-op: This interrupt exists as a hook point for TSRs.
        // The interrupt handler simply returns, allowing any chained
        // handlers (from TSRs) to execute.
    }
}
