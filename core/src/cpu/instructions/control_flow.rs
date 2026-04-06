use crate::{
    bus::Bus,
    cpu::{Cpu, CpuType, cpu_flag, timing},
};

impl Cpu {
    /// HLT - Halt (opcode F4)
    /// Stops instruction execution until a hardware interrupt occurs
    pub(in crate::cpu) fn hlt(&mut self, bus: &mut Bus) {
        self.halted = true;

        // HLT: 2 cycles
        bus.increment_cycle_count(timing::cycles::HLT)
    }

    /// INT - Software Interrupt (opcode CD)
    /// Calls interrupt handler
    pub(in crate::cpu) fn int(&mut self, bus: &mut Bus) {
        let int_num = self.fetch_byte(bus);
        self.dispatch_interrupt(bus, int_num);
        bus.increment_cycle_count(timing::cycles::INT)
    }

    /// IRET - Interrupt Return (opcode CF)
    /// Returns from interrupt handler
    pub(in crate::cpu) fn iret(&mut self, bus: &mut Bus) {
        // Pop IP, CS, and flags
        let new_ip = self.pop(bus);
        let new_cs = self.pop(bus);
        let new_flags = self.pop(bus);

        let old_if = self.get_flag(cpu_flag::INTERRUPT);

        // Check for inter-privilege return in protected mode
        let target_rpl = new_cs & 0x03;
        let inter_privilege = self.in_protected_mode() && target_rpl > self.cpl as u16;

        self.ip = new_ip;
        if self.in_protected_mode() {
            self.load_segment_register(1, new_cs, bus);
        } else {
            self.set_cs_real(new_cs);
        }
        self.flags = match self.cpu_type {
            CpuType::I8086 => (new_flags & 0x0FFF) | 0xF002,
            CpuType::I80286 => (new_flags & 0x0FFF) | 0x0002,
            _ => (new_flags & 0x7FFF) | 0x0002,
        };

        // Inter-privilege return: pop SS:SP from stack to restore outer ring stack
        if inter_privilege {
            let new_sp = self.pop(bus);
            let new_ss = self.pop(bus);
            self.load_segment_register(2, new_ss, bus); // SS
            self.sp = new_sp;
            self.cpl = target_rpl as u8;
        }

        if self.exec_logging_enabled {
            log::info!("popped IP={:04X}", self.ip);
            log::info!("popped CS={:04X}", self.cs);
            log::info!(
                "popped flags={:04X}  CF={} PF={} AF={} ZF={} SF={} TF={} IF={}->{} DF={} OF={}",
                self.flags,
                self.get_flag(cpu_flag::CARRY) as u8,
                self.get_flag(cpu_flag::PARITY) as u8,
                self.get_flag(cpu_flag::AUXILIARY) as u8,
                self.get_flag(cpu_flag::ZERO) as u8,
                self.get_flag(cpu_flag::SIGN) as u8,
                self.get_flag(cpu_flag::TRAP) as u8,
                old_if as u8,
                self.get_flag(cpu_flag::INTERRUPT) as u8,
                self.get_flag(cpu_flag::DIRECTION) as u8,
                self.get_flag(cpu_flag::OVERFLOW) as u8,
            );
        }

        // IRET: 24 cycles
        bus.increment_cycle_count(timing::cycles::IRET)
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
        bus.increment_cycle_count(timing::cycles::CALL_NEAR_DIRECT);
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
                bus.increment_cycle_count(if mode == 0b11 {
                    timing::cycles::CALL_NEAR_INDIRECT_REG
                } else {
                    timing::cycles::CALL_NEAR_INDIRECT_MEM
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                });
            }
            3 => {
                // CALL m16:16 (far indirect)
                if mode == 0b11 {
                    panic!("Far CALL indirect requires bus operand");
                }
                let offset = bus.memory_read_u16(addr);
                let segment = bus.memory_read_u16(addr + 2);
                if self.in_protected_mode() {
                    self.far_call_pm(segment, offset, bus);
                } else {
                    self.push(self.cs, bus);
                    self.push(self.ip, bus);
                    self.ip = offset;
                    self.set_cs_real(segment);
                }

                // CALL far indirect: 37+EA cycles
                bus.increment_cycle_count(
                    timing::cycles::CALL_FAR_INDIRECT_MEM
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some()),
                );
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
        bus.increment_cycle_count(if opcode == 0xC2 {
            timing::cycles::RET_NEAR_POP
        } else {
            timing::cycles::RET_NEAR
        });
    }

    /// JMP short relative (opcode EB)
    /// Jump to IP + signed 8-bit displacement
    pub(in crate::cpu) fn jmp_short(&mut self, bus: &mut Bus) {
        let offset = self.fetch_byte(bus) as i8;
        self.ip = self.ip.wrapping_add(offset as i16 as u16);

        // JMP short: 15 cycles
        bus.increment_cycle_count(timing::cycles::JMP_SHORT)
    }

    /// JMP indirect (opcode FF)
    /// FF /4: JMP r/m16 (near indirect)
    /// FF /5: JMP m16:16 (far indirect)
    pub(in crate::cpu) fn jmp_indirect(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        match operation {
            4 => {
                // JMP r/m16 (near indirect)
                let offset = self.read_rm16(mode, rm, addr, bus);
                self.ip = offset;

                // JMP near indirect: 11 cycles (reg), 18+EA (mem)
                bus.increment_cycle_count(if mode == 0b11 {
                    timing::cycles::JMP_NEAR_INDIRECT_REG
                } else {
                    timing::cycles::JMP_NEAR_INDIRECT_MEM
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                });
            }
            5 => {
                // JMP m16:16 (far indirect)
                if mode == 0b11 {
                    panic!("Far JMP indirect requires bus operand");
                }
                let offset = bus.memory_read_u16(addr);
                let segment = bus.memory_read_u16(addr + 2);
                if self.in_protected_mode() {
                    self.far_jmp_pm(segment, offset, bus);
                } else {
                    self.ip = offset;
                    self.set_cs_real(segment);
                }

                // JMP far indirect: 24+EA cycles
                bus.increment_cycle_count(
                    timing::cycles::JMP_FAR_INDIRECT_MEM
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some()),
                );
            }
            _ => panic!("Invalid JMP indirect operation: {}", operation),
        }
    }

    /// Conditional jumps - short relative (opcodes 70-7F)
    /// Jump to IP + signed 8-bit displacement if condition is met
    pub(in crate::cpu) fn jmp_conditional(&mut self, opcode: u8, bus: &mut Bus) {
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
            bus.increment_cycle_count(timing::cycles::CONDITIONAL_JUMP_TAKEN)
        } else {
            // Conditional jump not taken: 4 cycles
            bus.increment_cycle_count(timing::cycles::CONDITIONAL_JUMP_NOT_TAKEN)
        }
    }

    /// CWD - Convert Word to Double word (opcode 99)
    /// Sign-extends AX into DX:AX
    pub(in crate::cpu) fn cwd(&mut self, bus: &mut Bus) {
        if (self.ax & 0x8000) != 0 {
            // Negative - extend with 1s
            self.dx = 0xFFFF;
        } else {
            // Positive - extend with 0s
            self.dx = 0x0000;
        }

        // CWD: 5 cycles
        bus.increment_cycle_count(timing::cycles::CWD)
    }

    /// LOOPE/LOOPZ - Loop while CX != 0 and ZF = 1 (opcode E1)
    /// Decrements CX and jumps if CX != 0 and ZF = 1
    pub(in crate::cpu) fn loope(&mut self, bus: &mut Bus) {
        let offset = self.fetch_byte(bus) as i8;
        self.cx = self.cx.wrapping_sub(1);
        if self.cx != 0 && self.get_flag(cpu_flag::ZERO) {
            self.ip = self.ip.wrapping_add(offset as i16 as u16);
            // LOOPE taken: 18 cycles
            bus.increment_cycle_count(timing::cycles::LOOPE_TAKEN)
        } else {
            // LOOPE not taken: 6 cycles
            bus.increment_cycle_count(timing::cycles::LOOPE_NOT_TAKEN)
        }
    }

    /// LOOPNE/LOOPNZ - Loop while CX != 0 and ZF = 0 (opcode E0)
    /// Decrements CX and jumps if CX != 0 and ZF = 0
    pub(in crate::cpu) fn loopne(&mut self, bus: &mut Bus) {
        let offset = self.fetch_byte(bus) as i8;
        self.cx = self.cx.wrapping_sub(1);
        if self.cx != 0 && !self.get_flag(cpu_flag::ZERO) {
            self.ip = self.ip.wrapping_add(offset as i16 as u16);
            // LOOPNE taken: 19 cycles
            bus.increment_cycle_count(timing::cycles::LOOPNE_TAKEN)
        } else {
            // LOOPNE not taken: 5 cycles
            bus.increment_cycle_count(timing::cycles::LOOPNE_NOT_TAKEN)
        }
    }

    /// CBW - Convert Byte to Word (opcode 98)
    /// Sign-extends AL into AX
    pub(in crate::cpu) fn cbw(&mut self, bus: &mut Bus) {
        let al = (self.ax & 0xFF) as i8;
        self.ax = al as i16 as u16;

        // CBW: 2 cycles
        bus.increment_cycle_count(timing::cycles::CBW)
    }

    /// LOOP - Loop while CX != 0 (opcode E2)
    /// Decrements CX and jumps if CX != 0
    pub(in crate::cpu) fn loop_inst(&mut self, bus: &mut Bus) {
        let offset = self.fetch_byte(bus) as i8;
        self.cx = self.cx.wrapping_sub(1);
        if self.cx != 0 {
            self.ip = self.ip.wrapping_add(offset as i16 as u16);
            // LOOP taken: 17 cycles
            bus.increment_cycle_count(timing::cycles::LOOP_TAKEN)
        } else {
            // LOOP not taken: 5 cycles
            bus.increment_cycle_count(timing::cycles::LOOP_NOT_TAKEN)
        }
    }

    /// JCXZ - Jump if CX is Zero (opcode E3)
    /// Jumps if CX = 0
    pub(in crate::cpu) fn jcxz(&mut self, bus: &mut Bus) {
        let offset = self.fetch_byte(bus) as i8;
        if self.cx == 0 {
            self.ip = self.ip.wrapping_add(offset as i16 as u16);
            // JCXZ taken: 18 cycles
            bus.increment_cycle_count(timing::cycles::JCXZ_TAKEN)
        } else {
            // JCXZ not taken: 6 cycles
            bus.increment_cycle_count(timing::cycles::JCXZ_NOT_TAKEN)
        }
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
                let addr = self.seg_offset_to_phys(self.ss, self.bp, bus);
                let val = bus.memory_read_u16(addr);
                self.push(val, bus);
            }
            // Push current frame's display entry
            self.push(frame_temp, bus);
        }

        // Set new frame pointer and allocate locals
        self.bp = frame_temp;
        self.sp = self.sp.wrapping_sub(size);

        bus.increment_cycle_count(if level == 0 {
            timing::cycles::ENTER_LEVEL0
        } else {
            timing::cycles::ENTER_LEVEL_BASE + timing::cycles::ENTER_LEVEL_PER * level as u32
        });
    }

    /// LEAVE - High Level Procedure Exit (opcode C9, 80186+)
    /// Tears down the stack frame created by ENTER.
    /// Equivalent to: MOV SP, BP / POP BP
    pub(in crate::cpu) fn leave(&mut self, bus: &mut Bus) {
        // Restore SP to frame pointer, then pop caller's BP
        self.sp = self.bp;
        self.bp = self.pop(bus);

        bus.increment_cycle_count(timing::cycles::LEAVE);
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
        self.set_cs_real(segment);

        // INT 3: 52 cycles
        bus.increment_cycle_count(timing::cycles::INT3)
    }

    /// JMP near relative (opcode E9)
    /// Jump to IP + signed 16-bit displacement
    pub(in crate::cpu) fn jmp_near(&mut self, bus: &mut Bus) {
        let offset = self.fetch_word(bus) as i16;
        self.ip = self.ip.wrapping_add(offset as u16);

        // JMP near direct: 15 cycles
        bus.increment_cycle_count(timing::cycles::JMP_NEAR_DIRECT)
    }

    /// JMP far direct (opcode EA)
    /// Jump to far address (segment:offset)
    pub(in crate::cpu) fn jmp_far(&mut self, bus: &mut Bus) {
        let offset = self.fetch_word(bus);
        let segment = self.fetch_word(bus);
        if self.in_protected_mode() {
            self.far_jmp_pm(segment, offset, bus);
        } else {
            self.ip = offset;
            self.set_cs_real(segment);
        }

        // JMP far direct: 15 cycles
        bus.increment_cycle_count(timing::cycles::JMP_FAR_DIRECT)
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
        let cs_val = self.pop(bus);

        // Check for inter-privilege return in protected mode
        let target_rpl = cs_val & 0x03;
        let inter_privilege = self.in_protected_mode() && target_rpl > self.cpl as u16;

        if self.in_protected_mode() {
            self.load_segment_register(1, cs_val, bus);
        } else {
            self.set_cs_real(cs_val);
        }

        // Add the immediate to SP (if CA)
        self.sp = self.sp.wrapping_add(bytes_to_pop);

        // Inter-privilege return: pop SS:SP to restore outer ring stack
        if inter_privilege {
            let new_sp = self.pop(bus);
            let new_ss = self.pop(bus);
            self.load_segment_register(2, new_ss, bus); // SS
            self.sp = new_sp;
            self.cpl = target_rpl as u8;
        }

        // RET far: 18 cycles (CB), 17 cycles (CA with pop)
        bus.increment_cycle_count(if opcode == 0xCA {
            timing::cycles::RET_FAR_POP
        } else {
            timing::cycles::RET_FAR
        });
    }

    /// CALL far direct (opcode 9A)
    /// Call far procedure
    pub(in crate::cpu) fn call_far(&mut self, bus: &mut Bus) {
        let offset = self.fetch_word(bus);
        let segment = self.fetch_word(bus);
        if self.in_protected_mode() {
            self.far_call_pm(segment, offset, bus);
        } else {
            self.push(self.cs, bus);
            self.push(self.ip, bus);
            self.ip = offset;
            self.set_cs_real(segment);
        }

        // CALL far direct: 28 cycles
        bus.increment_cycle_count(timing::cycles::CALL_FAR_DIRECT);
    }
}
