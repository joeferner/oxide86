use super::super::{Cpu, FLAG_CARRY, FLAG_OVERFLOW};
use crate::memory::Memory;

impl Cpu {
    /// AND r/m and register (opcodes 20-23)
    /// 20: AND r/m8, r8
    /// 21: AND r/m16, r16
    /// 22: AND r8, r/m8
    /// 23: AND r16, r/m16
    pub(in crate::cpu) fn and_rm_reg(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0;

        let modrm = self.fetch_byte(memory);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        if is_word {
            let src = if dir {
                self.read_rm16(mode, rm, addr, memory)
            } else {
                self.get_reg16(reg)
            };
            let dst = if dir {
                self.get_reg16(reg)
            } else {
                self.read_rm16(mode, rm, addr, memory)
            };
            let result = dst & src;

            if dir {
                self.set_reg16(reg, result);
            } else {
                self.write_rm16(mode, rm, addr, result, memory);
            }

            self.set_flags_16(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        } else {
            let src = if dir {
                self.read_rm8(mode, rm, addr, memory)
            } else {
                self.get_reg8(reg)
            };
            let dst = if dir {
                self.get_reg8(reg)
            } else {
                self.read_rm8(mode, rm, addr, memory)
            };
            let result = dst & src;

            if dir {
                self.set_reg8(reg, result);
            } else {
                self.write_rm8(mode, rm, addr, result, memory);
            }

            self.set_flags_8(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        }
    }

    /// AND immediate to accumulator (opcodes 24-25)
    /// 24: AND AL, imm8
    /// 25: AND AX, imm16
    pub(in crate::cpu) fn and_imm_acc(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            let imm = self.fetch_word(memory);
            self.ax &= imm;
            self.set_flags_16(self.ax);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        } else {
            let imm = self.fetch_byte(memory);
            let al = (self.ax & 0xFF) as u8;
            let result = al & imm;
            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        }
    }

    /// OR r/m and register (opcodes 08-0B)
    /// 08: OR r/m8, r8
    /// 09: OR r/m16, r16
    /// 0A: OR r8, r/m8
    /// 0B: OR r16, r/m16
    pub(in crate::cpu) fn or_rm_reg(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0;

        let modrm = self.fetch_byte(memory);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        if is_word {
            let src = if dir {
                self.read_rm16(mode, rm, addr, memory)
            } else {
                self.get_reg16(reg)
            };
            let dst = if dir {
                self.get_reg16(reg)
            } else {
                self.read_rm16(mode, rm, addr, memory)
            };
            let result = dst | src;

            if dir {
                self.set_reg16(reg, result);
            } else {
                self.write_rm16(mode, rm, addr, result, memory);
            }

            self.set_flags_16(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        } else {
            let src = if dir {
                self.read_rm8(mode, rm, addr, memory)
            } else {
                self.get_reg8(reg)
            };
            let dst = if dir {
                self.get_reg8(reg)
            } else {
                self.read_rm8(mode, rm, addr, memory)
            };
            let result = dst | src;

            if dir {
                self.set_reg8(reg, result);
            } else {
                self.write_rm8(mode, rm, addr, result, memory);
            }

            self.set_flags_8(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        }
    }

    /// OR immediate to accumulator (opcodes 0C-0D)
    /// 0C: OR AL, imm8
    /// 0D: OR AX, imm16
    pub(in crate::cpu) fn or_imm_acc(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            let imm = self.fetch_word(memory);
            self.ax |= imm;
            self.set_flags_16(self.ax);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        } else {
            let imm = self.fetch_byte(memory);
            let al = (self.ax & 0xFF) as u8;
            let result = al | imm;
            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        }
    }

    /// XOR r/m and register (opcodes 30-33)
    /// 30: XOR r/m8, r8
    /// 31: XOR r/m16, r16
    /// 32: XOR r8, r/m8
    /// 33: XOR r16, r/m16
    pub(in crate::cpu) fn xor_rm_reg(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0;

        let modrm = self.fetch_byte(memory);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        if is_word {
            let src = if dir {
                self.read_rm16(mode, rm, addr, memory)
            } else {
                self.get_reg16(reg)
            };
            let dst = if dir {
                self.get_reg16(reg)
            } else {
                self.read_rm16(mode, rm, addr, memory)
            };
            let result = dst ^ src;

            if dir {
                self.set_reg16(reg, result);
            } else {
                self.write_rm16(mode, rm, addr, result, memory);
            }

            self.set_flags_16(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        } else {
            let src = if dir {
                self.read_rm8(mode, rm, addr, memory)
            } else {
                self.get_reg8(reg)
            };
            let dst = if dir {
                self.get_reg8(reg)
            } else {
                self.read_rm8(mode, rm, addr, memory)
            };
            let result = dst ^ src;

            if dir {
                self.set_reg8(reg, result);
            } else {
                self.write_rm8(mode, rm, addr, result, memory);
            }

            self.set_flags_8(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        }
    }

    /// XOR immediate to accumulator (opcodes 34-35)
    /// 34: XOR AL, imm8
    /// 35: XOR AX, imm16
    pub(in crate::cpu) fn xor_imm_acc(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            let imm = self.fetch_word(memory);
            self.ax ^= imm;
            self.set_flags_16(self.ax);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        } else {
            let imm = self.fetch_byte(memory);
            let al = (self.ax & 0xFF) as u8;
            let result = al ^ imm;
            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        }
    }

    /// TEST r/m and register (opcodes 84-85)
    /// 84: TEST r/m8, r8
    /// 85: TEST r/m16, r16
    pub(in crate::cpu) fn test_rm_reg(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;

        let modrm = self.fetch_byte(memory);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        if is_word {
            let src = self.get_reg16(reg);
            let dst = self.read_rm16(mode, rm, addr, memory);
            let result = dst & src;

            self.set_flags_16(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        } else {
            let src = self.get_reg8(reg);
            let dst = self.read_rm8(mode, rm, addr, memory);
            let result = dst & src;

            self.set_flags_8(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        }
    }

    /// TEST immediate to accumulator (opcodes A8-A9)
    /// A8: TEST AL, imm8
    /// A9: TEST AX, imm16
    pub(in crate::cpu) fn test_imm_acc(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            let imm = self.fetch_word(memory);
            let result = self.ax & imm;
            self.set_flags_16(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        } else {
            let imm = self.fetch_byte(memory);
            let al = (self.ax & 0xFF) as u8;
            let result = al & imm;
            self.set_flags_8(result);
            self.set_flag(FLAG_CARRY, false);
            self.set_flag(FLAG_OVERFLOW, false);
        }
    }

    /// NOT/NEG/MUL/DIV/TEST Group 3 (opcode 0xF6 for 8-bit, 0xF7 for 16-bit)
    /// F6 /0: TEST r/m8, imm8
    /// F6 /2: NOT r/m8
    /// F7 /0: TEST r/m16, imm16
    /// F7 /2: NOT r/m16
    pub(in crate::cpu) fn unary_group3(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;
        let modrm = self.fetch_byte(memory);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        match operation {
            0 => {
                // TEST r/m, imm
                if is_word {
                    let imm = self.fetch_word(memory);
                    let value = self.read_rm16(mode, rm, addr, memory);
                    let result = value & imm;
                    self.set_flags_16(result);
                    self.set_flag(FLAG_CARRY, false);
                    self.set_flag(FLAG_OVERFLOW, false);
                } else {
                    let imm = self.fetch_byte(memory);
                    let value = self.read_rm8(mode, rm, addr, memory);
                    let result = value & imm;
                    self.set_flags_8(result);
                    self.set_flag(FLAG_CARRY, false);
                    self.set_flag(FLAG_OVERFLOW, false);
                }
            }
            2 => {
                // NOT
                if is_word {
                    let value = self.read_rm16(mode, rm, addr, memory);
                    self.write_rm16(mode, rm, addr, !value, memory);
                } else {
                    let value = self.read_rm8(mode, rm, addr, memory);
                    self.write_rm8(mode, rm, addr, !value, memory);
                }
                // NOT doesn't affect flags
            }
            _ => panic!("Unimplemented Group 3 operation: {}", operation),
        }
    }
}
