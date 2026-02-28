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

    /// JMP near relative (opcode E9)
    /// Jump to IP + signed 16-bit displacement
    pub(in crate::cpu) fn jmp_near(&mut self, bus: &Bus) {
        let offset = self.fetch_word(bus) as i16;
        self.ip = self.ip.wrapping_add(offset as u16);

        // JMP near direct: 15 cycles
        self.last_instruction_cycles = timing::cycles::JMP_NEAR_DIRECT;
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



    /// JMP far direct (opcode EA)
    /// Jump to far address (segment:offset)
    pub(in crate::cpu) fn jmp_far(&mut self, bus: &Bus) {
        let offset = self.fetch_word(bus);
        let segment = self.fetch_word(bus);
        self.ip = offset;
        self.cs = segment;

        // JMP far direct: 15 cycles
        self.last_instruction_cycles = timing::cycles::JMP_FAR_DIRECT;
    }

    /// CALL far direct (opcode 9A)
    /// Call far procedure
    pub(in crate::cpu) fn call_far(&mut self, bus: &mut Bus) {
        let offset = self.fetch_word(bus);
        let segment = self.fetch_word(bus);
        // Push CS and IP
        self.push(self.cs, bus);
        self.push(self.ip, bus);
        // Jump to far address
        self.ip = offset;
        self.cs = segment;

        // CALL far direct: 28 cycles
        self.last_instruction_cycles = timing::cycles::CALL_FAR_DIRECT;
    }

    /// RET far (opcodes CA, CB)
    /// CA: RET far with imm16 (pop additional bytes)
    /// CB: RET far
    pub(in crate::cpu) fn retf(&mut self, opcode: u8, bus: &mut Bus) {
        // If opcode is CA, fetch the immediate BEFORE popping
        // (fetch_word reads from CS:IP which will change after pops)
        let bytes_to_pop = if opcode == 0xCA {
            self.fetch_word(bus)
        } else {
            0
        };

        // Pop IP and CS
        self.ip = self.pop(bus);
        self.cs = self.pop(bus);

        // Add the immediate to SP (if CA)
        self.sp = self.sp.wrapping_add(bytes_to_pop);

        // RET far: 18 cycles (CB), 17 cycles (CA with pop)
        self.last_instruction_cycles = if opcode == 0xCA {
            timing::cycles::RET_FAR_POP
        } else {
            timing::cycles::RET_FAR
        };
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
