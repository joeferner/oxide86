use crate::{
    cpu::{Cpu, cpu_flag, timing},
    memory_bus::MemoryBus,
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
    pub(in crate::cpu) fn int(&mut self, bus: &mut MemoryBus) {
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
        let offset = bus.read_u16(ivt_addr);
        let segment = bus.read_u16(ivt_addr + 2);
        self.ip = offset;
        self.cs = segment;

        // INT: 51 cycles
        self.last_instruction_cycles = timing::cycles::INT;
    }

    /// IRET - Interrupt Return (opcode CF)
    /// Returns from interrupt handler
    pub(in crate::cpu) fn iret(&mut self, memory_bus: &mut MemoryBus) {
        // Pop IP, CS, and flags
        let new_ip = self.pop(memory_bus);
        let new_cs = self.pop(memory_bus);
        let new_flags = self.pop(memory_bus);

        self.ip = new_ip;
        self.cs = new_cs;
        // 8086 behavior: only allow bits 0-11 to be modified, force bit 1 to 1
        self.flags = (new_flags & 0x0FFF) | 0x0002;

        // IRET: 24 cycles
        self.last_instruction_cycles = timing::cycles::IRET;
    }

    /// CALL near relative (opcode E8)
    /// Call procedure at IP + signed 16-bit offset
    pub(in crate::cpu) fn call_near(&mut self, memory_bus: &mut MemoryBus) {
        let offset = self.fetch_word(memory_bus) as i16;
        // Push return address (current IP after reading offset)
        self.push(self.ip, memory_bus);
        // Jump to target
        self.ip = self.ip.wrapping_add(offset as u16);

        // CALL near direct: 19 cycles
        self.last_instruction_cycles = timing::cycles::CALL_NEAR_DIRECT;
    }

    /// CALL indirect (opcode FF)
    /// FF /2: CALL r/m16 (near indirect)
    /// FF /3: CALL m16:16 (far indirect)
    pub(in crate::cpu) fn call_indirect(&mut self, memory_bus: &mut MemoryBus) {
        let modrm = self.fetch_byte(memory_bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, memory_bus);

        match operation {
            2 => {
                // CALL r/m16 (near indirect)
                let offset = self.read_rm16(mode, rm, addr, memory_bus);
                self.push(self.ip, memory_bus);
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
                let offset = memory_bus.read_u16(addr);
                let segment = memory_bus.read_u16(addr + 2);
                self.push(self.cs, memory_bus);
                self.push(self.ip, memory_bus);
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
    pub(in crate::cpu) fn ret(&mut self, opcode: u8, memory_bus: &mut MemoryBus) {
        // If opcode is C2, fetch the immediate BEFORE popping
        // (fetch_word reads from CS:IP which will change after pop)
        let bytes_to_pop = if opcode == 0xC2 {
            self.fetch_word(memory_bus)
        } else {
            0
        };

        // Pop return address
        self.ip = self.pop(memory_bus);

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
    pub(in crate::cpu) fn jmp_short(&mut self, memory_bus: &MemoryBus) {
        let offset = self.fetch_byte(memory_bus) as i8;
        self.ip = self.ip.wrapping_add(offset as i16 as u16);

        // JMP short: 15 cycles
        self.last_instruction_cycles = timing::cycles::JMP_SHORT;
    }

    /// JMP indirect (opcode FF)
    /// FF /4: JMP r/m16 (near indirect)
    /// FF /5: JMP m16:16 (far indirect)
    pub(in crate::cpu) fn jmp_indirect(&mut self, memory_bus: &MemoryBus) {
        let modrm = self.fetch_byte(memory_bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, memory_bus);

        match operation {
            4 => {
                // JMP r/m16 (near indirect)
                let offset = self.read_rm16(mode, rm, addr, memory_bus);
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
                let offset = memory_bus.read_u16(addr);
                let segment = memory_bus.read_u16(addr + 2);
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
    pub(in crate::cpu) fn jmp_conditional(&mut self, opcode: u8, memory_bus: &MemoryBus) {
        let offset = self.fetch_byte(memory_bus) as i8;

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
}
