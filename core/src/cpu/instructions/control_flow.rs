use super::super::{Cpu, cpu_flag, timing};
use crate::Bus;

impl Cpu {
    /// HLT - Halt (opcode F4)
    /// Stops instruction execution until a hardware interrupt occurs
    pub(in crate::cpu) fn hlt(&mut self) {
        self.halted = true;

        // HLT: 2 cycles
        self.last_instruction_cycles = timing::cycles::HLT;
    }


    /// LEAVE - High Level Procedure Exit (opcode C9, 80186+)
    /// Tears down the stack frame created by ENTER.
    /// Equivalent to: MOV SP, BP / POP BP
    pub(in crate::cpu) fn leave(&mut self, bus: &Bus) {
        // Restore SP to frame pointer, then pop caller's BP
        self.sp = self.bp;
        self.bp = self.pop(bus);

        self.last_instruction_cycles = timing::cycles::LEAVE;
    }

    /// INTO - Interrupt on Overflow (opcode CE)
    /// Calls interrupt 4 if OF is set
    pub(in crate::cpu) fn into(&mut self, bus: &mut Bus) {
        if self.get_flag(cpu_flag::OVERFLOW) {
            self.push(self.flags, bus);
            self.push(self.cs, bus);
            self.push(self.ip, bus);
            self.set_flag(cpu_flag::INTERRUPT, false);
            self.set_flag(cpu_flag::TRAP, false);
            let ivt_addr = 4 * 4;
            let offset = bus.read_u16(ivt_addr);
            let segment = bus.read_u16(ivt_addr + 2);
            self.ip = offset;
            self.cs = segment;
            // INTO taken: 53 cycles
            self.last_instruction_cycles = timing::cycles::INTO_TAKEN;
        } else {
            // INTO not taken: 4 cycles
            self.last_instruction_cycles = timing::cycles::INTO_NOT_TAKEN;
        }
    }
}
