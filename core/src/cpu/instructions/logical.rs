use super::super::{Cpu, cpu_flag, timing};
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
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
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

    /// AND immediate to accumulator (opcodes 24-25)
    /// 24: AND AL, imm8
    /// 25: AND AX, imm16
    pub(in crate::cpu) fn and_imm_acc(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            let imm = self.fetch_word(memory);
            self.ax &= imm;
            self.set_flags_16(self.ax);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        } else {
            let imm = self.fetch_byte(memory);
            let al = (self.ax & 0xFF) as u8;
            let result = al & imm;
            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        }

        // AND immediate to accumulator: 4 cycles
        self.last_instruction_cycles = timing::cycles::AND_IMM_ACC;
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
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
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

    /// OR immediate to accumulator (opcodes 0C-0D)
    /// 0C: OR AL, imm8
    /// 0D: OR AX, imm16
    pub(in crate::cpu) fn or_imm_acc(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            let imm = self.fetch_word(memory);
            self.ax |= imm;
            self.set_flags_16(self.ax);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        } else {
            let imm = self.fetch_byte(memory);
            let al = (self.ax & 0xFF) as u8;
            let result = al | imm;
            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        }

        // OR immediate to accumulator: 4 cycles
        self.last_instruction_cycles = timing::cycles::OR_IMM_ACC;
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
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
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
    pub(in crate::cpu) fn xor_imm_acc(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            let imm = self.fetch_word(memory);
            self.ax ^= imm;
            self.set_flags_16(self.ax);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        } else {
            let imm = self.fetch_byte(memory);
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
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        } else {
            let src = self.get_reg8(reg);
            let dst = self.read_rm8(mode, rm, addr, memory);
            let result = dst & src;

            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        }

        // TEST r/m, reg: 3 cycles (reg), 9+EA (mem)
        self.last_instruction_cycles = if mode == 0b11 {
            timing::cycles::TEST_REG_REG
        } else {
            timing::cycles::TEST_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
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
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        } else {
            let imm = self.fetch_byte(memory);
            let al = (self.ax & 0xFF) as u8;
            let result = al & imm;
            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flag(cpu_flag::OVERFLOW, false);
        }

        // TEST immediate to accumulator: 4 cycles
        self.last_instruction_cycles = timing::cycles::TEST_IMM_ACC;
    }

    /// NOT/NEG/MUL/DIV/TEST Group 3 (opcode 0xF6 for 8-bit, 0xF7 for 16-bit)
    /// F6 /0: TEST r/m8, imm8
    /// F6 /2: NOT r/m8
    /// F6 /3: NEG r/m8
    /// F6 /4: MUL r/m8
    /// F6 /5: IMUL r/m8
    /// F6 /6: DIV r/m8
    /// F6 /7: IDIV r/m8
    /// F7 /0: TEST r/m16, imm16
    /// F7 /2: NOT r/m16
    /// F7 /3: NEG r/m16
    /// F7 /4: MUL r/m16
    /// F7 /5: IMUL r/m16
    /// F7 /6: DIV r/m16
    /// F7 /7: IDIV r/m16
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
                    self.set_flag(cpu_flag::CARRY, false);
                    self.set_flag(cpu_flag::OVERFLOW, false);
                } else {
                    let imm = self.fetch_byte(memory);
                    let value = self.read_rm8(mode, rm, addr, memory);
                    let result = value & imm;
                    self.set_flags_8(result);
                    self.set_flag(cpu_flag::CARRY, false);
                    self.set_flag(cpu_flag::OVERFLOW, false);
                }
                // TEST r/m, imm: 5 cycles (reg), 11+EA (mem)
                self.last_instruction_cycles = if mode == 0b11 {
                    timing::cycles::TEST_IMM_REG
                } else {
                    timing::cycles::TEST_IMM_MEM
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                };
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
                // NOT: 3 cycles (reg), 16+EA (mem)
                self.last_instruction_cycles = if mode == 0b11 {
                    timing::cycles::NOT_REG
                } else {
                    timing::cycles::NOT_MEM
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                };
            }
            3 => {
                // NEG (two's complement negation)
                if is_word {
                    let value = self.read_rm16(mode, rm, addr, memory);
                    let result = value.wrapping_neg();
                    self.write_rm16(mode, rm, addr, result, memory);
                    self.set_flags_16(result);
                    self.set_flag(cpu_flag::CARRY, value != 0);
                    self.set_flag(cpu_flag::OVERFLOW, value == 0x8000);
                    // Auxiliary carry for lower nibble
                    self.set_flag(cpu_flag::AUXILIARY, (value & 0x0F) != 0);
                } else {
                    let value = self.read_rm8(mode, rm, addr, memory);
                    let result = value.wrapping_neg();
                    self.write_rm8(mode, rm, addr, result, memory);
                    self.set_flags_8(result);
                    self.set_flag(cpu_flag::CARRY, value != 0);
                    self.set_flag(cpu_flag::OVERFLOW, value == 0x80);
                    // Auxiliary carry for lower nibble
                    self.set_flag(cpu_flag::AUXILIARY, (value & 0x0F) != 0);
                }
                // NEG: 3 cycles (reg), 16+EA (mem)
                self.last_instruction_cycles = if mode == 0b11 {
                    timing::cycles::NEG_REG
                } else {
                    timing::cycles::NEG_MEM
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                };
            }
            4 => {
                // MUL (unsigned multiply)
                if is_word {
                    let value = self.read_rm16(mode, rm, addr, memory);
                    let result = (self.ax as u32) * (value as u32);
                    self.ax = result as u16;
                    self.dx = (result >> 16) as u16;
                    // CF and OF are set if upper half (DX) is non-zero
                    let upper_non_zero = self.dx != 0;
                    self.set_flag(cpu_flag::CARRY, upper_non_zero);
                    self.set_flag(cpu_flag::OVERFLOW, upper_non_zero);
                    // Other flags are undefined, but we'll leave them as is
                } else {
                    let value = self.read_rm8(mode, rm, addr, memory);
                    let al = (self.ax & 0xFF) as u8;
                    let result = (al as u16) * (value as u16);
                    self.ax = result;
                    // CF and OF are set if upper half (AH) is non-zero
                    let upper_non_zero = (result >> 8) != 0;
                    self.set_flag(cpu_flag::CARRY, upper_non_zero);
                    self.set_flag(cpu_flag::OVERFLOW, upper_non_zero);
                }
                // MUL: 70-77 cycles (8-bit reg), 76-83+EA (8-bit mem), 118-133 (16-bit reg), 124-139+EA (16-bit mem)
                self.last_instruction_cycles = if is_word {
                    if mode == 0b11 {
                        timing::cycles::MUL_REG16
                    } else {
                        timing::cycles::MUL_MEM16
                            + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                    }
                } else if mode == 0b11 {
                    timing::cycles::MUL_REG8
                } else {
                    timing::cycles::MUL_MEM8
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                };
            }
            5 => {
                // IMUL (signed multiply)
                if is_word {
                    let value = self.read_rm16(mode, rm, addr, memory) as i16;
                    let result = (self.ax as i16 as i32) * (value as i32);
                    self.ax = result as u16;
                    self.dx = (result >> 16) as u16;
                    // CF and OF are set if result can't fit in lower half
                    // i.e., if sign extension of AX != DX:AX
                    let sign_extended = (self.ax as i16) as i32;
                    let overflow = sign_extended != result;
                    self.set_flag(cpu_flag::CARRY, overflow);
                    self.set_flag(cpu_flag::OVERFLOW, overflow);
                } else {
                    let value = self.read_rm8(mode, rm, addr, memory) as i8;
                    let al = (self.ax & 0xFF) as i8;
                    let result = (al as i16) * (value as i16);
                    self.ax = result as u16;
                    // CF and OF are set if result can't fit in AL
                    let sign_extended = ((self.ax & 0xFF) as i8) as i16;
                    let overflow = sign_extended != result;
                    self.set_flag(cpu_flag::CARRY, overflow);
                    self.set_flag(cpu_flag::OVERFLOW, overflow);
                }
                // IMUL: 80-98 cycles (8-bit reg), 86-104+EA (8-bit mem), 128-154 (16-bit reg), 134-160+EA (16-bit mem)
                self.last_instruction_cycles = if is_word {
                    if mode == 0b11 {
                        timing::cycles::IMUL_REG16
                    } else {
                        timing::cycles::IMUL_MEM16
                            + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                    }
                } else if mode == 0b11 {
                    timing::cycles::IMUL_REG8
                } else {
                    timing::cycles::IMUL_MEM8
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                };
            }
            6 => {
                // DIV (unsigned divide)
                if is_word {
                    let divisor = self.read_rm16(mode, rm, addr, memory) as u32;
                    if divisor == 0 {
                        log::warn!("DIV16: division by zero at {:04X}:{:04X}", self.cs, self.ip);
                        self.pending_exception = Some(0);
                        return;
                    }
                    let dividend = ((self.dx as u32) << 16) | (self.ax as u32);
                    let quotient = dividend / divisor;
                    let remainder = dividend % divisor;
                    if quotient > 0xFFFF {
                        log::warn!("DIV16: overflow at {:04X}:{:04X}", self.cs, self.ip);
                        self.pending_exception = Some(0);
                        return;
                    }
                    self.ax = quotient as u16;
                    self.dx = remainder as u16;
                    // Flags are undefined after DIV
                } else {
                    let divisor = self.read_rm8(mode, rm, addr, memory) as u16;
                    if divisor == 0 {
                        log::warn!("DIV8: division by zero at {:04X}:{:04X}", self.cs, self.ip);
                        self.pending_exception = Some(0);
                        return;
                    }
                    let dividend = self.ax;
                    let quotient: u16 = dividend / divisor;
                    let remainder: u16 = dividend % divisor;
                    if quotient > 0xFF {
                        log::warn!("DIV8: overflow at {:04X}:{:04X}", self.cs, self.ip);
                        self.pending_exception = Some(0);
                        return;
                    }
                    self.ax = (remainder << 8) | quotient;
                    // Flags are undefined after DIV
                }
                // DIV: 80-90 cycles (8-bit reg), 86-96+EA (8-bit mem), 144-162 (16-bit reg), 150-168+EA (16-bit mem)
                self.last_instruction_cycles = if is_word {
                    if mode == 0b11 {
                        timing::cycles::DIV_REG16
                    } else {
                        timing::cycles::DIV_MEM16
                            + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                    }
                } else if mode == 0b11 {
                    timing::cycles::DIV_REG8
                } else {
                    timing::cycles::DIV_MEM8
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                };
            }
            7 => {
                // IDIV (signed divide)
                if is_word {
                    let divisor = self.read_rm16(mode, rm, addr, memory) as i16 as i32;
                    if divisor == 0 {
                        log::warn!(
                            "IDIV16: division by zero at {:04X}:{:04X}",
                            self.cs,
                            self.ip
                        );
                        self.pending_exception = Some(0);
                        return;
                    }
                    let dividend = ((self.dx as i16 as i32) << 16) | (self.ax as i32);
                    let quotient = dividend / divisor;
                    let remainder = dividend % divisor;
                    if !(-32768..=32767).contains(&quotient) {
                        log::debug!("IDIV16: overflow at {:04X}:{:04X}", self.cs, self.ip);
                        self.pending_exception = Some(0);
                        return;
                    }
                    self.ax = quotient as u16;
                    self.dx = remainder as u16;
                    // Flags are undefined after IDIV
                } else {
                    let divisor = self.read_rm8(mode, rm, addr, memory) as i8 as i16;
                    if divisor == 0 {
                        log::warn!("IDIV8: division by zero at {:04X}:{:04X}", self.cs, self.ip);
                        self.pending_exception = Some(0);
                        return;
                    }
                    let dividend = self.ax as i16;
                    let quotient = dividend / divisor;
                    let remainder = dividend % divisor;
                    if !(-128..=127).contains(&quotient) {
                        log::warn!("IDIV8: overflow at {:04X}:{:04X}", self.cs, self.ip);
                        self.pending_exception = Some(0);
                        return;
                    }
                    let al = quotient as u8;
                    let ah = remainder as u8;
                    self.ax = ((ah as u16) << 8) | (al as u16);
                    // Flags are undefined after IDIV
                }
                // IDIV: 101-112 cycles (8-bit reg), 107-118+EA (8-bit mem), 165-184 (16-bit reg), 171-190+EA (16-bit mem)
                self.last_instruction_cycles = if is_word {
                    if mode == 0b11 {
                        timing::cycles::IDIV_REG16
                    } else {
                        timing::cycles::IDIV_MEM16
                            + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                    }
                } else if mode == 0b11 {
                    timing::cycles::IDIV_REG8
                } else {
                    timing::cycles::IDIV_MEM8
                        + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
                };
            }
            _ => panic!("Unimplemented Group 3 operation: {}", operation),
        }
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
