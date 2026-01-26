// INT 20h - Program Terminate
// This is the original DOS program termination interrupt.
// CS must contain the PSP segment when this interrupt is called.

use super::Cpu;

impl Cpu {
    /// INT 20h - Program Terminate
    /// Terminates the current program and returns control to the parent process.
    /// Note: CS must contain the PSP segment (this is handled automatically by
    /// COM programs since CS=PSP at start)
    pub(super) fn handle_int20(&mut self) {
        // Halt the CPU - same as INT 21h, AH=4Ch but without return code
        self.halted = true;
    }
}
