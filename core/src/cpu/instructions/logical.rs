use super::super::{Cpu, cpu_flag, timing};
use crate::Bus;

impl Cpu {
    /// AND r/m and register (opcodes 20-23)
    /// 20: AND r/m8, r8
    /// 21: AND r/m16, r16
    /// 22: AND r8, r/m8
    /// 23: AND r16, r/m16
    pub(in crate::cpu) fn and_rm_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0;

        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if is_word {
            let src = if dir {
                self.read_rm16(mode, rm, addr, bus)
            } else {
                self.get_reg16(reg)
            };
            let dst = if dir {
                self.get_reg16(reg)
            } else {
                self.read_rm16(mode, rm, addr, bus)
            };
            let result = dst & src;

            if dir {
                self.set_reg16(reg, result);
            } else {
                self.write_rm16(mode, rm, addr, result, bus);
            }

            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        } else {
            let src = if dir {
                self.read_rm8(mode, rm, addr, bus)
            } else {
                self.get_reg8(reg)
            };
            let dst = if dir {
                self.get_reg8(reg)
            } else {
                self.read_rm8(mode, rm, addr, bus)
            };
            let result = dst & src;

            if dir {
                self.set_reg8(reg, result);
            } else {
                self.write_rm8(mode, rm, addr, result, bus);
            }

            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        }

        // AND r/m, reg: 3 cycles (reg), 16+EA (mem to reg), 9+EA (reg to mem)
        self.last_instruction_cycles = if mode == 0b11 {
            timing::cycles::AND_REG_REG
        } else if dir {
            timing::cycles::AND_MEM_REG
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        } else {
            timing::cycles::AND_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// OR r/m and register (opcodes 08-0B)
    /// 08: OR r/m8, r8
    /// 09: OR r/m16, r16
    /// 0A: OR r8, r/m8
    /// 0B: OR r16, r/m16
    pub(in crate::cpu) fn or_rm_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0;

        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if is_word {
            let src = if dir {
                self.read_rm16(mode, rm, addr, bus)
            } else {
                self.get_reg16(reg)
            };
            let dst = if dir {
                self.get_reg16(reg)
            } else {
                self.read_rm16(mode, rm, addr, bus)
            };
            let result = dst | src;

            if dir {
                self.set_reg16(reg, result);
            } else {
                self.write_rm16(mode, rm, addr, result, bus);
            }

            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        } else {
            let src = if dir {
                self.read_rm8(mode, rm, addr, bus)
            } else {
                self.get_reg8(reg)
            };
            let dst = if dir {
                self.get_reg8(reg)
            } else {
                self.read_rm8(mode, rm, addr, bus)
            };
            let result = dst | src;

            if dir {
                self.set_reg8(reg, result);
            } else {
                self.write_rm8(mode, rm, addr, result, bus);
            }

            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        }

        // OR r/m, reg: 3 cycles (reg), 16+EA (mem to reg), 9+EA (reg to mem)
        self.last_instruction_cycles = if mode == 0b11 {
            timing::cycles::OR_REG_REG
        } else if dir {
            timing::cycles::OR_MEM_REG
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        } else {
            timing::cycles::OR_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }


    /// XOR r/m and register (opcodes 30-33)
    /// 30: XOR r/m8, r8
    /// 31: XOR r/m16, r16
    /// 32: XOR r8, r/m8
    /// 33: XOR r16, r/m16
    pub(in crate::cpu) fn xor_rm_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0;

        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if is_word {
            let src = if dir {
                self.read_rm16(mode, rm, addr, bus)
            } else {
                self.get_reg16(reg)
            };
            let dst = if dir {
                self.get_reg16(reg)
            } else {
                self.read_rm16(mode, rm, addr, bus)
            };
            let result = dst ^ src;

            if dir {
                self.set_reg16(reg, result);
            } else {
                self.write_rm16(mode, rm, addr, result, bus);
            }

            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        } else {
            let src = if dir {
                self.read_rm8(mode, rm, addr, bus)
            } else {
                self.get_reg8(reg)
            };
            let dst = if dir {
                self.get_reg8(reg)
            } else {
                self.read_rm8(mode, rm, addr, bus)
            };
            let result = dst ^ src;

            if dir {
                self.set_reg8(reg, result);
            } else {
                self.write_rm8(mode, rm, addr, result, bus);
            }

            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        }

        // XOR r/m, reg: 3 cycles (reg), 16+EA (mem to reg), 9+EA (reg to mem)
        self.last_instruction_cycles = if mode == 0b11 {
            timing::cycles::XOR_REG_REG
        } else if dir {
            timing::cycles::XOR_MEM_REG
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        } else {
            timing::cycles::XOR_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// XOR immediate to accumulator (opcodes 34-35)
    /// 34: XOR AL, imm8
    /// 35: XOR AX, imm16
    pub(in crate::cpu) fn xor_imm_acc(&mut self, opcode: u8, bus: &Bus) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            let imm = self.fetch_word(bus);
            self.ax ^= imm;
            self.set_flags_16(self.ax);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        } else {
            let imm = self.fetch_byte(bus);
            let al = (self.ax & 0xFF) as u8;
            let result = al ^ imm;
            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        }

        // XOR immediate to accumulator: 4 cycles
        self.last_instruction_cycles = timing::cycles::XOR_IMM_ACC;
    }

    /// CLC - Clear Carry Flag (opcode 0xF8)
    pub(in crate::cpu) fn clc(&mut self) {
        self.set_flag(cpu_flag::CARRY, false);

        // CLC: 2 cycles
        self.last_instruction_cycles = timing::cycles::FLAG_OPS;
    }

    /// STC - Set Carry Flag (opcode 0xF9)
    pub(in crate::cpu) fn stc(&mut self) {
        self.set_flag(cpu_flag::CARRY, true);

        // STC: 2 cycles
        self.last_instruction_cycles = timing::cycles::FLAG_OPS;
    }

    /// CMC - Complement Carry Flag (opcode 0xF5)
    pub(in crate::cpu) fn cmc(&mut self) {
        let carry = self.get_flag(cpu_flag::CARRY);
        self.set_flag(cpu_flag::CARRY, !carry);

        // CMC: 2 cycles
        self.last_instruction_cycles = timing::cycles::FLAG_OPS;
    }

    /// CLI - Clear Interrupt Flag (opcode 0xFA)
    pub(in crate::cpu) fn cli(&mut self) {
        self.set_flag(cpu_flag::INTERRUPT, false);

        // CLI: 2 cycles
        self.last_instruction_cycles = timing::cycles::FLAG_OPS;
    }

    /// STI - Set Interrupt Flag (opcode 0xFB)
    pub(in crate::cpu) fn sti(&mut self) {
        self.set_flag(cpu_flag::INTERRUPT, true);

        // STI: 2 cycles
        self.last_instruction_cycles = timing::cycles::FLAG_OPS;
    }
}
