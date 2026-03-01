use crate::{
    bus::Bus,
    cpu::{Cpu, cpu_flag, timing},
};

impl Cpu {
    /// Arithmetic with immediate to r/m (opcode 0x80)
    /// 80: Immediate Group 1 - ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m8, imm8
    pub(in crate::cpu) fn arith_imm8_rm8(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, bus);
        let imm = self.fetch_byte(bus);
        let dst = self.read_rm8(mode, rm, addr, bus);

        match operation {
            0 => {
                // ADD
                let (result, carry) = dst.overflowing_add(imm);
                let overflow = ((dst ^ result) & (imm ^ result) & 0x80) != 0;
                let aux_carry = ((dst & 0x0F) + (imm & 0x0F)) > 0x0F;
                self.write_rm8(mode, rm, addr, result, bus);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
                self.set_flag(cpu_flag::AUXILIARY, aux_carry);
            }
            1 => {
                // OR
                let result = dst | imm;
                self.write_rm8(mode, rm, addr, result, bus);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            2 => {
                // ADC
                let carry_in = if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 };
                let (temp, carry1) = dst.overflowing_add(imm);
                let (result, carry2) = temp.overflowing_add(carry_in);
                let carry = carry1 || carry2;
                let overflow = ((dst ^ result) & (imm ^ result) & 0x80) != 0;
                let aux_carry = ((dst & 0x0F) + (imm & 0x0F) + carry_in) > 0x0F;
                self.write_rm8(mode, rm, addr, result, bus);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
                self.set_flag(cpu_flag::AUXILIARY, aux_carry);
            }
            3 => {
                // SBB
                let borrow_in = if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 };
                let (temp, borrow1) = dst.overflowing_sub(imm);
                let (result, borrow2) = temp.overflowing_sub(borrow_in);
                let borrow = borrow1 || borrow2;
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x80) != 0;
                let aux_borrow = (dst & 0x0F) < ((imm & 0x0F) + borrow_in);
                self.write_rm8(mode, rm, addr, result, bus);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, borrow);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
                self.set_flag(cpu_flag::AUXILIARY, aux_borrow);
            }
            4 => {
                // AND
                let result = dst & imm;
                self.write_rm8(mode, rm, addr, result, bus);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            5 => {
                // SUB
                let (result, carry) = dst.overflowing_sub(imm);
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x80) != 0;
                let aux_carry = (dst & 0x0F) < (imm & 0x0F);
                self.write_rm8(mode, rm, addr, result, bus);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
                self.set_flag(cpu_flag::AUXILIARY, aux_carry);
            }
            6 => {
                // XOR
                let result = dst ^ imm;
                self.write_rm8(mode, rm, addr, result, bus);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            7 => {
                // CMP (like SUB but doesn't store result)
                let (result, carry) = dst.overflowing_sub(imm);
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x80) != 0;
                let aux_carry = (dst & 0x0F) < (imm & 0x0F);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
                self.set_flag(cpu_flag::AUXILIARY, aux_carry);
            }
            _ => panic!("Unimplemented arithmetic operation: {}", operation),
        }

        // Calculate cycle timing based on operation and operand type
        self.last_instruction_cycles = if mode == 0b11 {
            // Immediate to register: 4 cycles (all operations)
            4
        } else {
            // Immediate to memory: 17 + EA cycles (or 10+EA for CMP)
            let base = if operation == 7 { 10 } else { 17 };
            base + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// Arithmetic with sign-extended immediate to r/m (opcode 0x83)
    /// 83: Immediate Group 1 - ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m16, imm8 (sign-extended)
    pub(in crate::cpu) fn arith_imm8_rm(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, bus);
        let imm8 = self.fetch_byte(bus);
        // Sign-extend the 8-bit immediate to 16 bits
        let imm = if imm8 & 0x80 != 0 {
            0xFF00 | (imm8 as u16)
        } else {
            imm8 as u16
        };
        let dst = self.read_rm16(mode, rm, addr, bus);

        match operation {
            0 => {
                // ADD
                let (result, carry) = dst.overflowing_add(imm);
                let overflow = ((dst ^ result) & (imm ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            1 => {
                // OR
                let result = dst | imm;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            2 => {
                // ADC
                let carry_in = if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 };
                let (temp, carry1) = dst.overflowing_add(imm);
                let (result, carry2) = temp.overflowing_add(carry_in);
                let carry = carry1 || carry2;
                let overflow = ((dst ^ result) & (imm ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            3 => {
                // SBB
                let borrow_in = if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 };
                let (temp, borrow1) = dst.overflowing_sub(imm);
                let (result, borrow2) = temp.overflowing_sub(borrow_in);
                let borrow = borrow1 || borrow2;
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, borrow);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            4 => {
                // AND
                let result = dst & imm;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            5 => {
                // SUB
                let (result, carry) = dst.overflowing_sub(imm);
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            6 => {
                // XOR
                let result = dst ^ imm;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            7 => {
                // CMP (like SUB but doesn't store result)
                let (result, carry) = dst.overflowing_sub(imm);
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000) != 0;
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            _ => panic!("Unimplemented arithmetic operation: {}", operation),
        }

        // Calculate cycle timing (same as other arith_imm functions)
        self.last_instruction_cycles = if mode == 0b11 {
            4 // Immediate to register: 4 cycles (all operations)
        } else {
            let base = if operation == 7 { 10 } else { 17 };
            base + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// Arithmetic with immediate to r/m (opcode 0x81)
    /// 81: Immediate Group 1 - ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m16, imm16
    pub(in crate::cpu) fn arith_imm16_rm(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, bus);
        let imm = self.fetch_word(bus);
        let dst = self.read_rm16(mode, rm, addr, bus);

        match operation {
            0 => {
                // ADD
                let (result, carry) = dst.overflowing_add(imm);
                let overflow = ((dst ^ result) & (imm ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            1 => {
                // OR
                let result = dst | imm;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            2 => {
                // ADC
                let carry_in = if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 };
                let (temp, carry1) = dst.overflowing_add(imm);
                let (result, carry2) = temp.overflowing_add(carry_in);
                let carry = carry1 || carry2;
                let overflow = ((dst ^ result) & (imm ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            3 => {
                // SBB
                let borrow_in = if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 };
                let (temp, borrow1) = dst.overflowing_sub(imm);
                let (result, borrow2) = temp.overflowing_sub(borrow_in);
                let borrow = borrow1 || borrow2;
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, borrow);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            4 => {
                // AND
                let result = dst & imm;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            5 => {
                // SUB
                let (result, carry) = dst.overflowing_sub(imm);
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            6 => {
                // XOR
                let result = dst ^ imm;
                self.write_rm16(mode, rm, addr, result, bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            7 => {
                // CMP (like SUB but doesn't store result)
                let (result, carry) = dst.overflowing_sub(imm);
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000) != 0;
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            _ => panic!("Unimplemented arithmetic operation: {}", operation),
        }

        // Calculate cycle timing (same as arith_imm8_rm8)
        self.last_instruction_cycles = if mode == 0b11 {
            4 // Immediate to register: 4 cycles (all operations)
        } else {
            let base = if operation == 7 { 10 } else { 17 };
            base + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// INC/DEC r/m (opcode 0xFE for 8-bit, 0xFF for 16-bit)
    /// FE /0: INC r/m8
    /// FE /1: DEC r/m8
    /// FF /0: INC r/m16
    /// FF /1: DEC r/m16
    pub(in crate::cpu) fn inc_dec_rm(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let modrm = self.fetch_byte(bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        match operation {
            0 => {
                // INC
                if is_word {
                    let value = self.read_rm16(mode, rm, addr, bus);
                    let result = value.wrapping_add(1);
                    self.write_rm16(mode, rm, addr, result, bus);
                    self.set_flags_16(result);
                    let overflow = value == 0x7FFF;
                    self.set_flag(cpu_flag::OVERFLOW, overflow);
                    let aux_carry = (value & 0x0F) == 0x0F;
                    self.set_flag(cpu_flag::AUXILIARY, aux_carry);
                } else {
                    let value = self.read_rm8(mode, rm, addr, bus);
                    let result = value.wrapping_add(1);
                    self.write_rm8(mode, rm, addr, result, bus);
                    self.set_flags_8(result);
                    let overflow = value == 0x7F;
                    self.set_flag(cpu_flag::OVERFLOW, overflow);
                    let aux_carry = (value & 0x0F) == 0x0F;
                    self.set_flag(cpu_flag::AUXILIARY, aux_carry);
                }
            }
            1 => {
                // DEC
                if is_word {
                    let value = self.read_rm16(mode, rm, addr, bus);
                    let result = value.wrapping_sub(1);
                    self.write_rm16(mode, rm, addr, result, bus);
                    self.set_flags_16(result);
                    let overflow = value == 0x8000;
                    self.set_flag(cpu_flag::OVERFLOW, overflow);
                    let aux_carry = (value & 0x0F) == 0;
                    self.set_flag(cpu_flag::AUXILIARY, aux_carry);
                } else {
                    let value = self.read_rm8(mode, rm, addr, bus);
                    let result = value.wrapping_sub(1);
                    self.write_rm8(mode, rm, addr, result, bus);
                    self.set_flags_8(result);
                    let overflow = value == 0x80;
                    self.set_flag(cpu_flag::OVERFLOW, overflow);
                    let aux_carry = (value & 0x0F) == 0;
                    self.set_flag(cpu_flag::AUXILIARY, aux_carry);
                }
            }
            _ => panic!("Invalid INC/DEC operation: {}", operation),
        }

        // Calculate cycle timing
        self.last_instruction_cycles = if mode == 0b11 {
            // INC/DEC register: 2 cycles
            timing::cycles::INC_REG // Same timing for INC and DEC
        } else {
            // INC/DEC memory: 15 + EA cycles
            timing::cycles::INC_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// INC 16-bit register (opcodes 40-47)
    /// Increment register by 1 (does not affect carry flag)
    pub(in crate::cpu) fn inc_reg16(&mut self, opcode: u8) {
        let reg = opcode & 0x07;
        let value = self.get_reg16(reg);
        let result = value.wrapping_add(1);

        self.set_reg16(reg, result);
        self.set_flags_16(result);
        // INC does not affect the carry flag
        let overflow = value == 0x7FFF; // Overflow when going from max positive to negative
        self.set_flag(cpu_flag::OVERFLOW, overflow);
        let aux_carry = (value & 0x0F) == 0x0F;
        self.set_flag(cpu_flag::AUXILIARY, aux_carry);

        // INC register: 2 cycles
        self.last_instruction_cycles = timing::cycles::INC_REG;
    }

    /// DEC 16-bit register (opcodes 48-4F)
    /// Decrement register by 1 (does not affect carry flag)
    pub(in crate::cpu) fn dec_reg16(&mut self, opcode: u8) {
        let reg = opcode & 0x07;
        let value = self.get_reg16(reg);
        let result = value.wrapping_sub(1);

        self.set_reg16(reg, result);
        self.set_flags_16(result);
        // DEC does not affect the carry flag
        let overflow = value == 0x8000; // Overflow when going from min negative to positive
        self.set_flag(cpu_flag::OVERFLOW, overflow);
        let aux_carry = (value & 0x0F) == 0;
        self.set_flag(cpu_flag::AUXILIARY, aux_carry);

        // DEC register: 2 cycles
        self.last_instruction_cycles = timing::cycles::DEC_REG;
    }

    /// ADD r/m and register (opcodes 00-03)
    /// 00: ADD r/m8, r8
    /// 01: ADD r/m16, r16
    /// 02: ADD r8, r/m8
    /// 03: ADD r16, r/m16
    pub(in crate::cpu) fn add_rm_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if is_word {
            // 16-bit add
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

            let (result, carry) = dst.overflowing_add(src);
            let overflow = ((dst ^ result) & (src ^ result) & 0x8000) != 0;

            if dir {
                self.set_reg16(reg, result);
            } else {
                self.write_rm16(mode, rm, addr, result, bus);
            }

            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        } else {
            // 8-bit add
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

            let (result, carry) = dst.overflowing_add(src);
            let overflow = ((dst ^ result) & (src ^ result) & 0x80) != 0;
            let aux_carry = ((dst & 0x0F) + (src & 0x0F)) > 0x0F;

            if dir {
                self.set_reg8(reg, result);
            } else {
                self.write_rm8(mode, rm, addr, result, bus);
            }

            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
            self.set_flag(cpu_flag::AUXILIARY, aux_carry);
        }

        // Calculate cycle timing based on operands
        self.last_instruction_cycles = if mode == 0b11 {
            // ADD reg, reg: 3 cycles
            timing::cycles::ADD_REG_REG
        } else if dir {
            // ADD reg, mem: 9 + EA cycles
            timing::cycles::ADD_MEM_REG
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        } else {
            // ADD mem, reg: 16 + EA cycles
            timing::cycles::ADD_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// ADC r/m and register (opcodes 10-13)
    /// 10: ADC r/m8, r8
    /// 11: ADC r/m16, r16
    /// 12: ADC r8, r/m8
    /// 13: ADC r16, r/m16
    pub(in crate::cpu) fn adc_rm_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        let carry_in = if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 };

        if is_word {
            // 16-bit adc
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

            let (temp, carry1) = dst.overflowing_add(src);
            let (result, carry2) = temp.overflowing_add(carry_in);
            let carry = carry1 || carry2;
            let overflow = ((dst ^ result) & (src ^ result) & 0x8000) != 0;

            if dir {
                self.set_reg16(reg, result);
            } else {
                self.write_rm16(mode, rm, addr, result, bus);
            }

            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        } else {
            // 8-bit adc
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

            let (temp, carry1) = dst.overflowing_add(src);
            let (result, carry2) = temp.overflowing_add(carry_in as u8);
            let carry = carry1 || carry2;
            let overflow = ((dst ^ result) & (src ^ result) & 0x80) != 0;
            let aux_carry = ((dst & 0x0F) + (src & 0x0F) + (carry_in as u8)) > 0x0F;

            if dir {
                self.set_reg8(reg, result);
            } else {
                self.write_rm8(mode, rm, addr, result, bus);
            }

            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
            self.set_flag(cpu_flag::AUXILIARY, aux_carry);
        }

        // Calculate cycle timing (same pattern as ADD)
        self.last_instruction_cycles = if mode == 0b11 {
            timing::cycles::ADC_REG_REG // 3 cycles
        } else if dir {
            timing::cycles::ADC_MEM_REG +  // 9 + EA cycles
                timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        } else {
            timing::cycles::ADC_REG_MEM +  // 16 + EA cycles
                timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// SUB r/m and register (opcodes 28-2B)
    /// 28: SUB r/m8, r8
    /// 29: SUB r/m16, r16
    /// 2A: SUB r8, r/m8
    /// 2B: SUB r16, r/m16
    pub(in crate::cpu) fn sub_rm_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if is_word {
            // 16-bit sub
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

            let (result, carry) = dst.overflowing_sub(src);
            let overflow = ((dst ^ src) & (dst ^ result) & 0x8000) != 0;

            if dir {
                self.set_reg16(reg, result);
            } else {
                self.write_rm16(mode, rm, addr, result, bus);
            }

            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        } else {
            // 8-bit sub
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

            let (result, carry) = dst.overflowing_sub(src);
            let overflow = ((dst ^ src) & (dst ^ result) & 0x80) != 0;
            let aux_carry = (dst & 0x0F) < (src & 0x0F);

            if dir {
                self.set_reg8(reg, result);
            } else {
                self.write_rm8(mode, rm, addr, result, bus);
            }

            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
            self.set_flag(cpu_flag::AUXILIARY, aux_carry);
        }

        // Calculate cycle timing (same pattern as ADD)
        self.last_instruction_cycles = if mode == 0b11 {
            timing::cycles::SUB_REG_REG // 3 cycles
        } else if dir {
            timing::cycles::SUB_MEM_REG +  // 9 + EA cycles
                timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        } else {
            timing::cycles::SUB_REG_MEM +  // 16 + EA cycles
                timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// SBB r/m and register (opcodes 18-1B)
    /// 18: SBB r/m8, r8
    /// 19: SBB r/m16, r16
    /// 1A: SBB r8, r/m8
    /// 1B: SBB r16, r/m16
    pub(in crate::cpu) fn sbb_rm_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        let borrow_in = if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 };

        if is_word {
            // 16-bit sbb
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

            let (temp, borrow1) = dst.overflowing_sub(src);
            let (result, borrow2) = temp.overflowing_sub(borrow_in);
            let borrow = borrow1 || borrow2;
            let overflow = ((dst ^ src) & (dst ^ result) & 0x8000) != 0;

            if dir {
                self.set_reg16(reg, result);
            } else {
                self.write_rm16(mode, rm, addr, result, bus);
            }

            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, borrow);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        } else {
            // 8-bit sbb
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

            let (temp, borrow1) = dst.overflowing_sub(src);
            let (result, borrow2) = temp.overflowing_sub(borrow_in as u8);
            let borrow = borrow1 || borrow2;
            let overflow = ((dst ^ src) & (dst ^ result) & 0x80) != 0;
            let aux_borrow = (dst & 0x0F) < ((src & 0x0F) + (borrow_in as u8));

            if dir {
                self.set_reg8(reg, result);
            } else {
                self.write_rm8(mode, rm, addr, result, bus);
            }

            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, borrow);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
            self.set_flag(cpu_flag::AUXILIARY, aux_borrow);
        }

        // SBB r/m, reg: 3 cycles (reg), 16+EA (mem to reg), 9+EA (reg to mem)
        self.last_instruction_cycles = if mode == 0b11 {
            timing::cycles::SBB_REG_REG
        } else if dir {
            timing::cycles::SBB_MEM_REG
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        } else {
            timing::cycles::SBB_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// ADD immediate to accumulator (opcodes 04-05)
    /// 04: ADD AL, imm8
    /// 05: ADD AX, imm16
    pub(in crate::cpu) fn add_imm_acc(&mut self, opcode: u8, bus: &Bus) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            // ADD AX, imm16
            let imm = self.fetch_word(bus);
            let (result, carry) = self.ax.overflowing_add(imm);
            let overflow = ((self.ax ^ result) & (imm ^ result) & 0x8000) != 0;

            self.ax = result;
            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        } else {
            // ADD AL, imm8
            let imm = self.fetch_byte(bus);
            let al = (self.ax & 0xFF) as u8;
            let (result, carry) = al.overflowing_add(imm);
            let overflow = ((al ^ result) & (imm ^ result) & 0x80) != 0;
            let aux_carry = ((al & 0x0F) + (imm & 0x0F)) > 0x0F;

            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
            self.set_flag(cpu_flag::AUXILIARY, aux_carry);
        }

        // ADD immediate to accumulator: 4 cycles
        self.last_instruction_cycles = timing::cycles::ADD_IMM_ACC;
    }

    /// NEG - Two's Complement Negation (F6/F7 Group 3, operation 3)
    /// NEG r/m8 or NEG r/m16
    /// Handled in unary_group3 in logical.rs, but the logic belongs here conceptually
    /// DAA - Decimal Adjust After Addition (opcode 0x27)
    /// Adjusts AL after BCD addition
    pub(in crate::cpu) fn daa(&mut self) {
        let mut al = (self.ax & 0xFF) as u8;
        let old_al = al;
        let old_cf = self.get_flag(cpu_flag::CARRY);

        if (al & 0x0F) > 9 || self.get_flag(cpu_flag::AUXILIARY) {
            al = al.wrapping_add(6);
            self.set_flag(cpu_flag::AUXILIARY, true);
        } else {
            self.set_flag(cpu_flag::AUXILIARY, false);
        }

        if old_al > 0x99 || old_cf {
            al = al.wrapping_add(0x60);
            self.set_flag(cpu_flag::CARRY, true);
        } else {
            self.set_flag(cpu_flag::CARRY, false);
        }

        self.ax = (self.ax & 0xFF00) | al as u16;
        self.set_flags_8(al);

        // DAA: 4 cycles
        self.last_instruction_cycles = timing::cycles::DAA;
    }

    /// SUB immediate to accumulator (opcodes 2C-2D)
    /// 2C: SUB AL, imm8
    /// 2D: SUB AX, imm16
    pub(in crate::cpu) fn sub_imm_acc(&mut self, opcode: u8, bus: &Bus) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            // SUB AX, imm16
            let imm = self.fetch_word(bus);
            let (result, carry) = self.ax.overflowing_sub(imm);
            let overflow = ((self.ax ^ imm) & (self.ax ^ result) & 0x8000) != 0;

            self.ax = result;
            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        } else {
            // SUB AL, imm8
            let imm = self.fetch_byte(bus);
            let al = (self.ax & 0xFF) as u8;
            let (result, carry) = al.overflowing_sub(imm);
            let overflow = ((al ^ imm) & (al ^ result) & 0x80) != 0;
            let aux_carry = (al & 0x0F) < (imm & 0x0F);

            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
            self.set_flag(cpu_flag::AUXILIARY, aux_carry);
        }

        // SUB immediate to accumulator: 4 cycles
        self.last_instruction_cycles = timing::cycles::SUB_IMM_ACC;
    }

    /// DAS - Decimal Adjust After Subtraction (opcode 0x2F)
    /// Adjusts AL after BCD subtraction
    pub(in crate::cpu) fn das(&mut self) {
        let mut al = (self.ax & 0xFF) as u8;
        let old_al = al;
        let old_cf = self.get_flag(cpu_flag::CARRY);

        if (al & 0x0F) > 9 || self.get_flag(cpu_flag::AUXILIARY) {
            al = al.wrapping_sub(6);
            self.set_flag(cpu_flag::AUXILIARY, true);
        } else {
            self.set_flag(cpu_flag::AUXILIARY, false);
        }

        if old_al > 0x99 || old_cf {
            al = al.wrapping_sub(0x60);
            self.set_flag(cpu_flag::CARRY, true);
        } else {
            self.set_flag(cpu_flag::CARRY, false);
        }

        self.ax = (self.ax & 0xFF00) | al as u16;
        self.set_flags_8(al);

        // DAS: 4 cycles
        self.last_instruction_cycles = timing::cycles::DAS;
    }

    /// AAA - ASCII Adjust After Addition (opcode 0x37)
    /// Adjusts AL and AH after unpacked BCD addition
    pub(in crate::cpu) fn aaa(&mut self) {
        let al = (self.ax & 0xFF) as u8;
        if (al & 0x0F) > 9 || self.get_flag(cpu_flag::AUXILIARY) {
            self.ax = self.ax.wrapping_add(0x106);
            self.set_flag(cpu_flag::AUXILIARY, true);
            self.set_flag(cpu_flag::CARRY, true);
        } else {
            self.set_flag(cpu_flag::AUXILIARY, false);
            self.set_flag(cpu_flag::CARRY, false);
        }
        self.ax &= 0xFF0F; // Clear high nibble of AL

        // AAA: 4 cycles
        self.last_instruction_cycles = timing::cycles::AAA;
    }

    /// AAS - ASCII Adjust After Subtraction (opcode 0x3F)
    /// Adjusts AL and AH after unpacked BCD subtraction
    pub(in crate::cpu) fn aas(&mut self) {
        let al = (self.ax & 0xFF) as u8;
        if (al & 0x0F) > 9 || self.get_flag(cpu_flag::AUXILIARY) {
            self.ax = self.ax.wrapping_sub(6);
            let ah = ((self.ax >> 8) as u8).wrapping_sub(1);
            self.ax = ((ah as u16) << 8) | (self.ax & 0xFF);
            self.set_flag(cpu_flag::AUXILIARY, true);
            self.set_flag(cpu_flag::CARRY, true);
        } else {
            self.set_flag(cpu_flag::AUXILIARY, false);
            self.set_flag(cpu_flag::CARRY, false);
        }
        self.ax &= 0xFF0F; // Clear high nibble of AL

        // AAS: 4 cycles
        self.last_instruction_cycles = timing::cycles::AAS;
    }

    /// AAM - ASCII Adjust After Multiplication (opcode 0xD4)
    /// Converts binary product in AL to unpacked BCD in AX
    pub(in crate::cpu) fn aam(&mut self, bus: &Bus) {
        let base = self.fetch_byte(bus); // Usually 0x0A (10), but can be customized
        let al = (self.ax & 0xFF) as u8;
        if base == 0 {
            panic!("Division by zero in AAM instruction");
        }
        let ah = al / base;
        let new_al = al % base;
        self.ax = ((ah as u16) << 8) | (new_al as u16);
        self.set_flags_8(new_al);
        self.set_flag(cpu_flag::PARITY, self.ax.count_ones().is_multiple_of(2));

        // AAM: 83 cycles
        self.last_instruction_cycles = timing::cycles::AAM;
    }

    /// AAD - ASCII Adjust Before Division (opcode 0xD5)
    /// Converts unpacked BCD in AX to binary in AL
    pub(in crate::cpu) fn aad(&mut self, bus: &Bus) {
        let base = self.fetch_byte(bus); // Usually 0x0A (10), but can be customized
        let al = (self.ax & 0xFF) as u8;
        let ah = ((self.ax >> 8) & 0xFF) as u8;
        let result = al.wrapping_add(ah.wrapping_mul(base));
        self.ax = (self.ax & 0xFF00) | (result as u16);
        // Clear AH
        self.ax &= 0x00FF;
        self.set_flags_8(result);

        // AAD: 60 cycles
        self.last_instruction_cycles = timing::cycles::AAD;
    }

    /// ADC immediate to accumulator (opcodes 14-15)
    /// 14: ADC AL, imm8
    /// 15: ADC AX, imm16
    pub(in crate::cpu) fn adc_imm_acc(&mut self, opcode: u8, bus: &Bus) {
        let is_word = opcode & 0x01 != 0;
        let carry_in = if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 };

        if is_word {
            // ADC AX, imm16
            let imm = self.fetch_word(bus);
            let (temp, carry1) = self.ax.overflowing_add(imm);
            let (result, carry2) = temp.overflowing_add(carry_in);
            let carry = carry1 || carry2;
            let overflow = ((self.ax ^ result) & (imm ^ result) & 0x8000) != 0;

            self.ax = result;
            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        } else {
            // ADC AL, imm8
            let imm = self.fetch_byte(bus);
            let al = (self.ax & 0xFF) as u8;
            let (temp, carry1) = al.overflowing_add(imm);
            let (result, carry2) = temp.overflowing_add(carry_in as u8);
            let carry = carry1 || carry2;
            let overflow = ((al ^ result) & (imm ^ result) & 0x80) != 0;
            let aux_carry = ((al & 0x0F) + (imm & 0x0F) + (carry_in as u8)) > 0x0F;

            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
            self.set_flag(cpu_flag::AUXILIARY, aux_carry);
        }

        // ADC immediate to accumulator: 4 cycles
        self.last_instruction_cycles = timing::cycles::ADC_IMM_ACC;
    }

    /// SBB immediate to accumulator (opcodes 1C-1D)
    /// 1C: SBB AL, imm8
    /// 1D: SBB AX, imm16
    pub(in crate::cpu) fn sbb_imm_acc(&mut self, opcode: u8, bus: &Bus) {
        let is_word = opcode & 0x01 != 0;
        let borrow_in = if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 };

        if is_word {
            // SBB AX, imm16
            let imm = self.fetch_word(bus);
            let (temp, borrow1) = self.ax.overflowing_sub(imm);
            let (result, borrow2) = temp.overflowing_sub(borrow_in);
            let borrow = borrow1 || borrow2;
            let overflow = ((self.ax ^ imm) & (self.ax ^ result) & 0x8000) != 0;

            self.ax = result;
            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, borrow);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        } else {
            // SBB AL, imm8
            let imm = self.fetch_byte(bus);
            let al = (self.ax & 0xFF) as u8;
            let (temp, borrow1) = al.overflowing_sub(imm);
            let (result, borrow2) = temp.overflowing_sub(borrow_in as u8);
            let borrow = borrow1 || borrow2;
            let overflow = ((al ^ imm) & (al ^ result) & 0x80) != 0;
            let aux_borrow = (al & 0x0F) < ((imm & 0x0F) + (borrow_in as u8));

            self.ax = (self.ax & 0xFF00) | result as u16;
            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, borrow);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
            self.set_flag(cpu_flag::AUXILIARY, aux_borrow);
        }

        // SBB immediate to accumulator: 4 cycles
        self.last_instruction_cycles = timing::cycles::SBB_IMM_ACC;
    }
}
