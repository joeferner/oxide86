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
}
