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

    /// ENTER - Make Stack Frame (opcode C8, 80186+)
    /// Creates a procedure stack frame for high-level language support.
    /// Encoding: C8 iw ib (imm16 = local frame size, imm8 = nesting level 0-31)
    pub(in crate::cpu) fn enter(&mut self, bus: &mut Bus) {
        let size = self.fetch_word(bus);
        let level = (self.fetch_byte(bus) & 0x1F) as u16;

        // Push caller's frame pointer
        self.push(self.bp, bus);

        // frame_temp = SP after push (address of saved BP; becomes new BP)
        let frame_temp = self.sp;

        // For nested procedures (Pascal-style), push display entries
        if level > 0 {
            for _ in 1..level {
                // Walk caller's display chain (BP still holds caller's BP)
                self.bp = self.bp.wrapping_sub(2);
                let addr = Self::physical_address(self.ss, self.bp);
                let val = bus.read_u16(addr);
                self.push(val, bus);
            }
            // Push current frame's display entry
            self.push(frame_temp, bus);
        }

        // Set new frame pointer and allocate locals
        self.bp = frame_temp;
        self.sp = self.sp.wrapping_sub(size);

        self.last_instruction_cycles = if level == 0 {
            timing::cycles::ENTER_LEVEL0
        } else {
            timing::cycles::ENTER_LEVEL_BASE + timing::cycles::ENTER_LEVEL_PER * level as u64
        };
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

    /// LOOP - Loop while CX != 0 (opcode E2)
    /// Decrements CX and jumps if CX != 0
    pub(in crate::cpu) fn loop_inst(&mut self, bus: &Bus) {
        let offset = self.fetch_byte(bus) as i8;
        self.cx = self.cx.wrapping_sub(1);
        if self.cx != 0 {
            self.ip = self.ip.wrapping_add(offset as i16 as u16);
            // LOOP taken: 17 cycles
            self.last_instruction_cycles = timing::cycles::LOOP_TAKEN;
        } else {
            // LOOP not taken: 5 cycles
            self.last_instruction_cycles = timing::cycles::LOOP_NOT_TAKEN;
        }
    }

    /// LOOPE/LOOPZ - Loop while CX != 0 and ZF = 1 (opcode E1)
    /// Decrements CX and jumps if CX != 0 and ZF = 1
    pub(in crate::cpu) fn loope(&mut self, bus: &Bus) {
        let offset = self.fetch_byte(bus) as i8;
        self.cx = self.cx.wrapping_sub(1);
        if self.cx != 0 && self.get_flag(cpu_flag::ZERO) {
            self.ip = self.ip.wrapping_add(offset as i16 as u16);
            // LOOPE taken: 18 cycles
            self.last_instruction_cycles = timing::cycles::LOOPE_TAKEN;
        } else {
            // LOOPE not taken: 6 cycles
            self.last_instruction_cycles = timing::cycles::LOOPE_NOT_TAKEN;
        }
    }

    /// LOOPNE/LOOPNZ - Loop while CX != 0 and ZF = 0 (opcode E0)
    /// Decrements CX and jumps if CX != 0 and ZF = 0
    pub(in crate::cpu) fn loopne(&mut self, bus: &Bus) {
        let offset = self.fetch_byte(bus) as i8;
        self.cx = self.cx.wrapping_sub(1);
        if self.cx != 0 && !self.get_flag(cpu_flag::ZERO) {
            self.ip = self.ip.wrapping_add(offset as i16 as u16);
            // LOOPNE taken: 19 cycles
            self.last_instruction_cycles = timing::cycles::LOOPNE_TAKEN;
        } else {
            // LOOPNE not taken: 5 cycles
            self.last_instruction_cycles = timing::cycles::LOOPNE_NOT_TAKEN;
        }
    }

    /// JCXZ - Jump if CX is Zero (opcode E3)
    /// Jumps if CX = 0
    pub(in crate::cpu) fn jcxz(&mut self, bus: &Bus) {
        let offset = self.fetch_byte(bus) as i8;
        if self.cx == 0 {
            self.ip = self.ip.wrapping_add(offset as i16 as u16);
            // JCXZ taken: 18 cycles
            self.last_instruction_cycles = timing::cycles::JCXZ_TAKEN;
        } else {
            // JCXZ not taken: 6 cycles
            self.last_instruction_cycles = timing::cycles::JCXZ_NOT_TAKEN;
        }
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

    /// INT 3 - Breakpoint Interrupt (opcode CC)
    /// Single-byte interrupt for breakpoints
    pub(in crate::cpu) fn int3(&mut self, bus: &mut Bus) {
        // Same as INT 3, but single byte opcode
        self.push(self.flags, bus);
        self.push(self.cs, bus);
        self.push(self.ip, bus);
        self.set_flag(cpu_flag::INTERRUPT, false);
        self.set_flag(cpu_flag::TRAP, false);
        let ivt_addr = 3 * 4;
        let offset = bus.read_u16(ivt_addr);
        let segment = bus.read_u16(ivt_addr + 2);
        self.ip = offset;
        self.cs = segment;

        // INT 3: 52 cycles
        self.last_instruction_cycles = timing::cycles::INT3;
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

    /// CBW - Convert Byte to Word (opcode 98)
    /// Sign-extends AL into AX
    pub(in crate::cpu) fn cbw(&mut self) {
        let al = (self.ax & 0xFF) as i8;
        self.ax = al as i16 as u16;

        // CBW: 2 cycles
        self.last_instruction_cycles = timing::cycles::CBW;
    }

    /// CWD - Convert Word to Double word (opcode 99)
    /// Sign-extends AX into DX:AX
    pub(in crate::cpu) fn cwd(&mut self) {
        if (self.ax & 0x8000) != 0 {
            // Negative - extend with 1s
            self.dx = 0xFFFF;
        } else {
            // Positive - extend with 0s
            self.dx = 0x0000;
        }

        // CWD: 5 cycles
        self.last_instruction_cycles = timing::cycles::CWD;
    }

    /// ESC - Escape to coprocessor (opcodes D8-DF)
    /// Passes instruction to 8087 FPU. Without a coprocessor, this is a NOP
    /// that reads the ModR/M byte and any displacement to maintain bus timing.
    pub(in crate::cpu) fn esc(&mut self, bus: &Bus) {
        let modrm = self.fetch_byte(bus);
        // Decode ModR/M to consume any displacement bytes
        let _ = self.decode_modrm(modrm, bus);
        // No operation - 8087 coprocessor not emulated

        // ESC: 2 cycles (no coprocessor)
        self.last_instruction_cycles = timing::cycles::ESC;
    }
}
