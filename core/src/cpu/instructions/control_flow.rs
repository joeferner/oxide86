use crate::{
    bus::Bus,
    cpu::{Cpu, cpu_flag, timing},
    physical_address,
};

impl Cpu {
    /// HLT - Halt (opcode F4)
    /// Stops instruction execution until a hardware interrupt occurs
    pub(in crate::cpu) fn hlt(&mut self) {
        self.halted = true;

        // HLT: 2 cycles
        self.last_instruction_cycles = timing::cycles::HLT;
    }

    /// INT - Software Interrupt (opcode CD)
    /// Calls interrupt handler
    pub(in crate::cpu) fn int(&mut self, bus: &mut Bus) {
        let int_num = self.fetch_byte(bus);

        // Push flags, CS, and IP
        self.push(self.flags, bus);
        self.push(self.cs, bus);
        self.push(self.ip, bus);
        // Clear IF and TF
        self.set_flag(cpu_flag::INTERRUPT, false);
        self.set_flag(cpu_flag::TRAP, false);
        // Load interrupt vector from interrupt vector table (IVT)
        // IVT starts at 0x00000, each entry is 4 bytes (offset, segment)
        let ivt_addr = (int_num as usize) * 4;
        let offset = bus.memory_read_u16(ivt_addr);
        let segment = bus.memory_read_u16(ivt_addr + 2);
        self.ip = offset;
        self.cs = segment;

        // INT: 51 cycles
        self.last_instruction_cycles = timing::cycles::INT;
    }

    /// IRET - Interrupt Return (opcode CF)
    /// Returns from interrupt handler
    pub(in crate::cpu) fn iret(&mut self, bus: &mut Bus) {
        // Pop IP, CS, and flags
        let new_ip = self.pop(bus);
        let new_cs = self.pop(bus);
        let new_flags = self.pop(bus);

        self.ip = new_ip;
        self.cs = new_cs;
        // 8086 behavior: only allow bits 0-11 to be modified, force bit 1 to 1
        self.flags = (new_flags & 0x0FFF) | 0x0002;

        // IRET: 24 cycles
        self.last_instruction_cycles = timing::cycles::IRET;
    }

    /// CALL near relative (opcode E8)
    /// Call procedure at IP + signed 16-bit offset
    pub(in crate::cpu) fn call_near(&mut self, bus: &mut Bus) {
        let offset = self.fetch_word(bus) as i16;
        // Push return address (current IP after reading offset)
        self.push(self.ip, bus);
        // Jump to target
        self.ip = self.ip.wrapping_add(offset as u16);

        // CALL near direct: 19 cycles
        self.last_instruction_cycles = timing::cycles::CALL_NEAR_DIRECT;
    }

    /// CALL indirect (opcode FF)
    /// FF /2: CALL r/m16 (near indirect)
    /// FF /3: CALL m16:16 (far indirect)
    pub(in crate::cpu) fn call_indirect(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        match operation {
            2 => {
                // CALL r/m16 (near indirect)
                let offset = self.read_rm16(mode, rm, addr, bus);
                self.push(self.ip, bus);
                self.ip = offset;

                // CALL near indirect: 16 cycles (reg), 21+EA (mem)
                self.last_instruction_cycles = if mode == 0b11 {
                    timing::cycles::CALL_NEAR_INDIRECT_REG
                } else {
                    timing::cycles::CALL_NEAR_INDIRECT_MEM
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                };
            }
            3 => {
                // CALL m16:16 (far indirect)
                if mode == 0b11 {
                    panic!("Far CALL indirect requires bus operand");
                }
                let offset = bus.memory_read_u16(addr);
                let segment = bus.memory_read_u16(addr + 2);
                self.push(self.cs, bus);
                self.push(self.ip, bus);
                self.ip = offset;
                self.cs = segment;

                // CALL far indirect: 37+EA cycles
                self.last_instruction_cycles = timing::cycles::CALL_FAR_INDIRECT_MEM
                    + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some());
            }
            _ => panic!("Invalid CALL indirect operation: {}", operation),
        }
    }

    /// RET (opcode C3: near return, C2: near return with imm16 pop)
    pub(in crate::cpu) fn ret(&mut self, opcode: u8, bus: &mut Bus) {
        // If opcode is C2, fetch the immediate BEFORE popping
        // (fetch_word reads from CS:IP which will change after pop)
        let bytes_to_pop = if opcode == 0xC2 {
            self.fetch_word(bus)
        } else {
            0
        };

        // Pop return address
        self.ip = self.pop(bus);

        // Add the immediate to SP (if C2)
        self.sp = self.sp.wrapping_add(bytes_to_pop);

        // RET near: 8 cycles (C3), 12 cycles (C2 with pop)
        self.last_instruction_cycles = if opcode == 0xC2 {
            timing::cycles::RET_NEAR_POP
        } else {
            timing::cycles::RET_NEAR
        };
    }

    /// JMP short relative (opcode EB)
    /// Jump to IP + signed 8-bit displacement
    pub(in crate::cpu) fn jmp_short(&mut self, bus: &Bus) {
        let offset = self.fetch_byte(bus) as i8;
        self.ip = self.ip.wrapping_add(offset as i16 as u16);

        // JMP short: 15 cycles
        self.last_instruction_cycles = timing::cycles::JMP_SHORT;
    }

    /// JMP indirect (opcode FF)
    /// FF /4: JMP r/m16 (near indirect)
    /// FF /5: JMP m16:16 (far indirect)
    pub(in crate::cpu) fn jmp_indirect(&mut self, bus: &Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        match operation {
            4 => {
                // JMP r/m16 (near indirect)
                let offset = self.read_rm16(mode, rm, addr, bus);
                self.ip = offset;

                // JMP near indirect: 11 cycles (reg), 18+EA (mem)
                self.last_instruction_cycles = if mode == 0b11 {
                    timing::cycles::JMP_NEAR_INDIRECT_REG
                } else {
                    timing::cycles::JMP_NEAR_INDIRECT_MEM
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                };
            }
            5 => {
                // JMP m16:16 (far indirect)
                if mode == 0b11 {
                    panic!("Far JMP indirect requires bus operand");
                }
                let offset = bus.memory_read_u16(addr);
                let segment = bus.memory_read_u16(addr + 2);
                self.ip = offset;
                self.cs = segment;

                // JMP far indirect: 24+EA cycles
                self.last_instruction_cycles = timing::cycles::JMP_FAR_INDIRECT_MEM
                    + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some());
            }
            _ => panic!("Invalid JMP indirect operation: {}", operation),
        }
    }

    /// Conditional jumps - short relative (opcodes 70-7F)
    /// Jump to IP + signed 8-bit displacement if condition is met
    pub(in crate::cpu) fn jmp_conditional(&mut self, opcode: u8, bus: &Bus) {
        let offset = self.fetch_byte(bus) as i8;

        let condition = match opcode {
            0x70 => self.get_flag(cpu_flag::OVERFLOW), // JO - Jump if overflow
            0x71 => !self.get_flag(cpu_flag::OVERFLOW), // JNO - Jump if not overflow
            0x72 => self.get_flag(cpu_flag::CARRY),    // JB/JC/JNAE - Jump if below/carry
            0x73 => !self.get_flag(cpu_flag::CARRY), // JAE/JNB/JNC - Jump if above or equal/not below/not carry
            0x74 => self.get_flag(cpu_flag::ZERO),   // JE/JZ - Jump if equal/zero
            0x75 => !self.get_flag(cpu_flag::ZERO),  // JNE/JNZ - Jump if not equal/not zero
            0x76 => self.get_flag(cpu_flag::CARRY) || self.get_flag(cpu_flag::ZERO), // JBE/JNA - Jump if below or equal/not above
            0x77 => !self.get_flag(cpu_flag::CARRY) && !self.get_flag(cpu_flag::ZERO), // JA/JNBE - Jump if above/not below or equal
            0x78 => self.get_flag(cpu_flag::SIGN), // JS - Jump if sign
            0x79 => !self.get_flag(cpu_flag::SIGN), // JNS - Jump if not sign
            0x7A => self.get_flag(cpu_flag::PARITY), // JP/JPE - Jump if parity/parity even
            0x7B => !self.get_flag(cpu_flag::PARITY), // JNP/JPO - Jump if not parity/parity odd
            0x7C => self.get_flag(cpu_flag::SIGN) != self.get_flag(cpu_flag::OVERFLOW), // JL/JNGE - Jump if less/not greater or equal
            0x7D => self.get_flag(cpu_flag::SIGN) == self.get_flag(cpu_flag::OVERFLOW), // JGE/JNL - Jump if greater or equal/not less
            0x7E => {
                self.get_flag(cpu_flag::ZERO)
                    || (self.get_flag(cpu_flag::SIGN) != self.get_flag(cpu_flag::OVERFLOW))
            } // JLE/JNG - Jump if less or equal/not greater
            0x7F => {
                !self.get_flag(cpu_flag::ZERO)
                    && (self.get_flag(cpu_flag::SIGN) == self.get_flag(cpu_flag::OVERFLOW))
            } // JG/JNLE - Jump if greater/not less or equal
            _ => unreachable!(),
        };

        if condition {
            self.ip = self.ip.wrapping_add(offset as i16 as u16);
            // Conditional jump taken: 16 cycles
            self.last_instruction_cycles = timing::cycles::CONDITIONAL_JUMP_TAKEN;
        } else {
            // Conditional jump not taken: 4 cycles
            self.last_instruction_cycles = timing::cycles::CONDITIONAL_JUMP_NOT_TAKEN;
        }
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

    /// CBW - Convert Byte to Word (opcode 98)
    /// Sign-extends AL into AX
    pub(in crate::cpu) fn cbw(&mut self) {
        let al = (self.ax & 0xFF) as i8;
        self.ax = al as i16 as u16;

        // CBW: 2 cycles
        self.last_instruction_cycles = timing::cycles::CBW;
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
                let addr = physical_address(self.ss, self.bp);
                let val = bus.memory_read_u16(addr);
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
        let offset = bus.memory_read_u16(ivt_addr);
        let segment = bus.memory_read_u16(ivt_addr + 2);
        self.ip = offset;
        self.cs = segment;

        // INT 3: 52 cycles
        self.last_instruction_cycles = timing::cycles::INT3;
    }

    /// JMP near relative (opcode E9)
    /// Jump to IP + signed 16-bit displacement
    pub(in crate::cpu) fn jmp_near(&mut self, bus: &Bus) {
        let offset = self.fetch_word(bus) as i16;
        self.ip = self.ip.wrapping_add(offset as u16);

        // JMP near direct: 15 cycles
        self.last_instruction_cycles = timing::cycles::JMP_NEAR_DIRECT;
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
}
