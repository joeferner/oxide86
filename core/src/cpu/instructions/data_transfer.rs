use super::super::Cpu;
use crate::memory::Memory;

impl Cpu {
    /// MOV immediate to register (opcodes B0-BF)
    /// B0-B7: MOV reg8, imm8
    /// B8-BF: MOV reg16, imm16
    pub(in crate::cpu) fn mov_imm_to_reg(&mut self, opcode: u8, memory: &Memory) {
        let reg = opcode & 0x07;
        let is_word = opcode & 0x08 != 0;

        if is_word {
            // 16-bit register
            let value = self.fetch_word(memory);
            self.set_reg16(reg, value);
        } else {
            // 8-bit register
            let value = self.fetch_byte(memory);
            self.set_reg8(reg, value);
        }
    }

    /// MOV register to/from r/m (opcodes 88-8B)
    /// 88: MOV r/m8, r8
    /// 89: MOV r/m16, r16
    /// 8A: MOV r8, r/m8
    /// 8B: MOV r16, r/m16
    pub(in crate::cpu) fn mov_reg_rm(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(memory);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        if is_word {
            // 16-bit move
            if dir {
                // MOV reg16, r/m16
                let value = self.read_rm16(mode, rm, addr, memory);
                self.set_reg16(reg, value);
            } else {
                // MOV r/m16, reg16
                let value = self.get_reg16(reg);
                self.write_rm16(mode, rm, addr, value, memory);
            }
        } else {
            // 8-bit move
            if dir {
                // MOV reg8, r/m8
                let value = self.read_rm8(mode, rm, addr, memory);
                self.set_reg8(reg, value);
            } else {
                // MOV r/m8, reg8
                let value = self.get_reg8(reg);
                self.write_rm8(mode, rm, addr, value, memory);
            }
        }
    }

    /// PUSH 16-bit register (opcodes 50-57)
    /// Push register onto stack
    pub(in crate::cpu) fn push_reg16(&mut self, opcode: u8, memory: &mut Memory) {
        let reg = opcode & 0x07;
        let value = self.get_reg16(reg);
        self.push(value, memory);
    }

    /// POP 16-bit register (opcodes 58-5F)
    /// Pop from stack to register
    pub(in crate::cpu) fn pop_reg16(&mut self, opcode: u8, memory: &mut Memory) {
        let reg = opcode & 0x07;
        let value = self.pop(memory);
        self.set_reg16(reg, value);
    }

    /// PUSH immediate (opcode 68: 16-bit, 6A: sign-extended 8-bit)
    pub(in crate::cpu) fn push_imm(&mut self, opcode: u8, memory: &mut Memory) {
        let value = if opcode == 0x68 {
            // PUSH imm16
            self.fetch_word(memory)
        } else {
            // PUSH imm8 (sign-extended to 16 bits)
            let imm8 = self.fetch_byte(memory);
            if imm8 & 0x80 != 0 {
                0xFF00 | (imm8 as u16)
            } else {
                imm8 as u16
            }
        };
        self.push(value, memory);
    }

    /// MOV immediate to r/m (opcodes C6-C7)
    /// C6: MOV r/m8, imm8
    /// C7: MOV r/m16, imm16
    pub(in crate::cpu) fn mov_imm_to_rm(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;
        let modrm = self.fetch_byte(memory);
        let (mode, _reg, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        // The reg field should be 0 for MOV immediate
        // (it's part of the opcode extension)

        if is_word {
            // MOV r/m16, imm16
            let value = self.fetch_word(memory);
            self.write_rm16(mode, rm, addr, value, memory);
        } else {
            // MOV r/m8, imm8
            let value = self.fetch_byte(memory);
            self.write_rm8(mode, rm, addr, value, memory);
        }
    }

    /// MOV accumulator to/from direct memory offset (opcodes A0-A3)
    /// A0: MOV AL, [moffs8] - Move byte at direct address to AL
    /// A1: MOV AX, [moffs16] - Move word at direct address to AX
    /// A2: MOV [moffs8], AL - Move AL to byte at direct address
    /// A3: MOV [moffs16], AX - Move AX to word at direct address
    pub(in crate::cpu) fn mov_acc_moffs(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;
        let to_acc = opcode & 0x02 == 0; // 0 = to accumulator, 1 = from accumulator

        // Fetch the direct memory offset (16-bit address)
        let offset = self.fetch_word(memory);
        let addr = Self::physical_address(self.ds, offset);

        if is_word {
            if to_acc {
                // MOV AX, [offset]
                self.ax = memory.read_word(addr);
            } else {
                // MOV [offset], AX
                memory.write_word(addr, self.ax);
            }
        } else {
            if to_acc {
                // MOV AL, [offset]
                let value = memory.read_byte(addr);
                self.ax = (self.ax & 0xFF00) | (value as u16);
            } else {
                // MOV [offset], AL
                let value = (self.ax & 0xFF) as u8;
                memory.write_byte(addr, value);
            }
        }
    }

    /// MOV segment register to r/m16 (opcode 8C)
    /// 8C: MOV r/m16, segreg
    /// Copies a segment register (ES, CS, SS, DS) to a 16-bit register or memory location
    pub(in crate::cpu) fn mov_segreg_to_rm(&mut self, memory: &mut Memory) {
        let modrm = self.fetch_byte(memory);
        let (mode, seg_reg, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        // The reg field specifies which segment register (ES=0, CS=1, SS=2, DS=3)
        let value = self.get_segreg(seg_reg);
        self.write_rm16(mode, rm, addr, value, memory);
    }

    /// MOV r/m16 to segment register (opcode 8E)
    /// 8E: MOV segreg, r/m16
    /// Copies a 16-bit register or memory value to a segment register (ES, CS, SS, DS)
    /// Note: MOV to CS is not recommended as it affects instruction fetching
    pub(in crate::cpu) fn mov_rm_to_segreg(&mut self, memory: &mut Memory) {
        let modrm = self.fetch_byte(memory);
        let (mode, seg_reg, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        // The reg field specifies which segment register (ES=0, CS=1, SS=2, DS=3)
        let value = self.read_rm16(mode, rm, addr, memory);
        self.set_segreg(seg_reg, value);
    }

    /// PUSH segment register (opcodes 06, 0E, 16, 1E)
    /// 06: PUSH ES
    /// 0E: PUSH CS
    /// 16: PUSH SS
    /// 1E: PUSH DS
    pub(in crate::cpu) fn push_segreg(&mut self, opcode: u8, memory: &mut Memory) {
        let seg = match opcode {
            0x06 => 0, // ES
            0x0E => 1, // CS
            0x16 => 2, // SS
            0x1E => 3, // DS
            _ => unreachable!(),
        };
        let value = self.get_segreg(seg);
        self.push(value, memory);
    }

    /// POP segment register (opcodes 07, 0F, 17, 1F)
    /// 07: POP ES
    /// 0F: POP CS (note: POP CS is unusual, typically not used)
    /// 17: POP SS
    /// 1F: POP DS
    pub(in crate::cpu) fn pop_segreg(&mut self, opcode: u8, memory: &mut Memory) {
        let seg = match opcode {
            0x07 => 0, // ES
            0x0F => 1, // CS
            0x17 => 2, // SS
            0x1F => 3, // DS
            _ => unreachable!(),
        };
        let value = self.pop(memory);
        self.set_segreg(seg, value);
    }

    /// PUSHF - Push Flags Register (opcode 9C)
    /// Pushes the FLAGS register onto the stack
    pub(in crate::cpu) fn pushf(&mut self, memory: &mut Memory) {
        self.push(self.flags, memory);
    }

    /// POPF - Pop Flags Register (opcode 9D)
    /// Pops a word from the stack into the FLAGS register
    pub(in crate::cpu) fn popf(&mut self, memory: &mut Memory) {
        self.flags = self.pop(memory);
    }

    /// POP r/m16 (opcode 8F) - Group 1A
    /// 8F /0: POP r/m16
    /// Pops a word from stack to register or memory location
    pub(in crate::cpu) fn pop_rm16(&mut self, memory: &mut Memory) {
        let modrm = self.fetch_byte(memory);
        let (mode, reg_field, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        // The reg field should be 0 for POP (it's an opcode extension)
        if reg_field != 0 {
            panic!("Invalid opcode extension for 8F: expected /0, got /{}", reg_field);
        }

        let value = self.pop(memory);
        self.write_rm16(mode, rm, addr, value, memory);
    }

    /// XCHG register with accumulator (opcodes 90-97)
    /// 90: NOP (XCHG AX, AX) - special case
    /// 91-97: XCHG AX, reg16
    pub(in crate::cpu) fn xchg_ax_reg(&mut self, opcode: u8) {
        let reg = opcode & 0x07;
        if reg == 0 {
            // NOP - XCHG AX, AX does nothing
            return;
        }
        let temp = self.ax;
        self.ax = self.get_reg16(reg);
        self.set_reg16(reg, temp);
    }

    /// XCHG register/memory with register (opcodes 86-87)
    /// 86: XCHG r/m8, r8
    /// 87: XCHG r/m16, r16
    pub(in crate::cpu) fn xchg_rm_reg(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;
        let modrm = self.fetch_byte(memory);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        if is_word {
            // 16-bit exchange
            let reg_val = self.get_reg16(reg);
            let rm_val = self.read_rm16(mode, rm, addr, memory);
            self.set_reg16(reg, rm_val);
            self.write_rm16(mode, rm, addr, reg_val, memory);
        } else {
            // 8-bit exchange
            let reg_val = self.get_reg8(reg);
            let rm_val = self.read_rm8(mode, rm, addr, memory);
            self.set_reg8(reg, rm_val);
            self.write_rm8(mode, rm, addr, reg_val, memory);
        }
    }

    /// XLAT - Table Look-up Translation (opcode 0xD7)
    /// Translates AL using lookup table at DS:BX
    /// AL = [DS:BX + AL]
    pub(in crate::cpu) fn xlat(&mut self, memory: &Memory) {
        let al = (self.ax & 0xFF) as u8;
        let offset = self.bx.wrapping_add(al as u16);
        let addr = Self::physical_address(self.ds, offset);
        let value = memory.read_byte(addr);
        self.ax = (self.ax & 0xFF00) | (value as u16);
    }

    /// LEA - Load Effective Address (opcode 0x8D)
    /// Loads the offset of the source operand into destination register
    pub(in crate::cpu) fn lea(&mut self, memory: &Memory) {
        let modrm = self.fetch_byte(memory);
        let mode = modrm >> 6;
        let reg = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;

        // LEA only works with memory operands (mode != 11)
        if mode == 0b11 {
            panic!("LEA cannot use register operand");
        }

        // Calculate the effective address offset (not physical address)
        let offset = match rm {
            0b000 => self.bx.wrapping_add(self.si),  // [BX + SI]
            0b001 => self.bx.wrapping_add(self.di),  // [BX + DI]
            0b010 => self.bp.wrapping_add(self.si),  // [BP + SI]
            0b011 => self.bp.wrapping_add(self.di),  // [BP + DI]
            0b100 => self.si,                         // [SI]
            0b101 => self.di,                         // [DI]
            0b110 => {
                if mode == 0b00 {
                    // Special case: direct address
                    self.fetch_word(memory)
                } else {
                    self.bp  // [BP]
                }
            }
            0b111 => self.bx,                         // [BX]
            _ => unreachable!(),
        };

        // Add displacement based on mode
        let effective_offset = match mode {
            0b00 => offset,  // No displacement (except for direct addressing handled above)
            0b01 => {
                // 8-bit signed displacement
                let disp = self.fetch_byte(memory) as i8;
                offset.wrapping_add(disp as i16 as u16)
            }
            0b10 => {
                // 16-bit displacement
                let disp = self.fetch_word(memory);
                offset.wrapping_add(disp)
            }
            _ => unreachable!(),
        };

        self.set_reg16(reg, effective_offset);
    }

    /// LDS - Load Pointer using DS (opcode 0xC5)
    /// Loads far pointer from memory into register and DS
    pub(in crate::cpu) fn lds(&mut self, memory: &Memory) {
        let modrm = self.fetch_byte(memory);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        // LDS only works with memory operands
        if mode == 0b11 {
            panic!("LDS cannot use register operand");
        }

        // Read offset and segment from memory (4 bytes total)
        let offset = memory.read_word(addr);
        let segment = memory.read_word(addr + 2);

        self.set_reg16(reg, offset);
        self.ds = segment;
    }

    /// LES - Load Pointer using ES (opcode 0xC4)
    /// Loads far pointer from memory into register and ES
    pub(in crate::cpu) fn les(&mut self, memory: &Memory) {
        let modrm = self.fetch_byte(memory);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        // LES only works with memory operands
        if mode == 0b11 {
            panic!("LES cannot use register operand");
        }

        // Read offset and segment from memory (4 bytes total)
        let offset = memory.read_word(addr);
        let segment = memory.read_word(addr + 2);

        self.set_reg16(reg, offset);
        self.es = segment;
    }

    /// LAHF - Load AH from Flags (opcode 0x9F)
    /// Loads SF, ZF, AF, PF, CF into AH
    pub(in crate::cpu) fn lahf(&mut self) {
        let ah = (self.flags & 0xFF) as u8;
        self.ax = (self.ax & 0x00FF) | ((ah as u16) << 8);
    }

    /// SAHF - Store AH into Flags (opcode 0x9E)
    /// Stores AH into SF, ZF, AF, PF, CF
    pub(in crate::cpu) fn sahf(&mut self) {
        let ah = ((self.ax >> 8) & 0xFF) as u8;
        // Only update lower 8 bits of flags (SF, ZF, 0, AF, 0, PF, 1, CF)
        // Preserve upper 8 bits
        self.flags = (self.flags & 0xFF00) | (ah as u16);
    }
}
