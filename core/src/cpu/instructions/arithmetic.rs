use crate::{
    cpu::{Cpu, cpu_flag, timing},
    memory_bus::MemoryBus,
};

impl Cpu {
    /// Arithmetic with immediate to r/m (opcode 0x80)
    /// 80: Immediate Group 1 - ADD/OR/ADC/SBB/AND/SUB/XOR/CMP r/m8, imm8
    pub(in crate::cpu) fn arith_imm8_rm8(&mut self, memory_bus: &mut MemoryBus) {
        let modrm = self.fetch_byte(memory_bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, memory_bus);
        let imm = self.fetch_byte(memory_bus);
        let dst = self.read_rm8(mode, rm, addr, memory_bus);

        match operation {
            0 => {
                // ADD
                let (result, carry) = dst.overflowing_add(imm);
                let overflow = ((dst ^ result) & (imm ^ result) & 0x80) != 0;
                let aux_carry = ((dst & 0x0F) + (imm & 0x0F)) > 0x0F;
                self.write_rm8(mode, rm, addr, result, memory_bus);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
                self.set_flag(cpu_flag::AUXILIARY, aux_carry);
            }
            1 => {
                // OR
                let result = dst | imm;
                self.write_rm8(mode, rm, addr, result, memory_bus);
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
                self.write_rm8(mode, rm, addr, result, memory_bus);
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
                self.write_rm8(mode, rm, addr, result, memory_bus);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, borrow);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
                self.set_flag(cpu_flag::AUXILIARY, aux_borrow);
            }
            4 => {
                // AND
                let result = dst & imm;
                self.write_rm8(mode, rm, addr, result, memory_bus);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            5 => {
                // SUB
                let (result, carry) = dst.overflowing_sub(imm);
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x80) != 0;
                let aux_carry = (dst & 0x0F) < (imm & 0x0F);
                self.write_rm8(mode, rm, addr, result, memory_bus);
                self.set_flags_8(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
                self.set_flag(cpu_flag::AUXILIARY, aux_carry);
            }
            6 => {
                // XOR
                let result = dst ^ imm;
                self.write_rm8(mode, rm, addr, result, memory_bus);
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
    pub(in crate::cpu) fn arith_imm8_rm(&mut self, memory_bus: &mut MemoryBus) {
        let modrm = self.fetch_byte(memory_bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, memory_bus);
        let imm8 = self.fetch_byte(memory_bus);
        // Sign-extend the 8-bit immediate to 16 bits
        let imm = if imm8 & 0x80 != 0 {
            0xFF00 | (imm8 as u16)
        } else {
            imm8 as u16
        };
        let dst = self.read_rm16(mode, rm, addr, memory_bus);

        match operation {
            0 => {
                // ADD
                let (result, carry) = dst.overflowing_add(imm);
                let overflow = ((dst ^ result) & (imm ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, memory_bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            1 => {
                // OR
                let result = dst | imm;
                self.write_rm16(mode, rm, addr, result, memory_bus);
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
                self.write_rm16(mode, rm, addr, result, memory_bus);
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
                self.write_rm16(mode, rm, addr, result, memory_bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, borrow);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            4 => {
                // AND
                let result = dst & imm;
                self.write_rm16(mode, rm, addr, result, memory_bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            5 => {
                // SUB
                let (result, carry) = dst.overflowing_sub(imm);
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, memory_bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            6 => {
                // XOR
                let result = dst ^ imm;
                self.write_rm16(mode, rm, addr, result, memory_bus);
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
    pub(in crate::cpu) fn arith_imm16_rm(&mut self, memory_bus: &mut MemoryBus) {
        let modrm = self.fetch_byte(memory_bus);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, memory_bus);
        let imm = self.fetch_word(memory_bus);
        let dst = self.read_rm16(mode, rm, addr, memory_bus);

        match operation {
            0 => {
                // ADD
                let (result, carry) = dst.overflowing_add(imm);
                let overflow = ((dst ^ result) & (imm ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, memory_bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            1 => {
                // OR
                let result = dst | imm;
                self.write_rm16(mode, rm, addr, result, memory_bus);
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
                self.write_rm16(mode, rm, addr, result, memory_bus);
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
                self.write_rm16(mode, rm, addr, result, memory_bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, borrow);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            4 => {
                // AND
                let result = dst & imm;
                self.write_rm16(mode, rm, addr, result, memory_bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, false);
                self.set_flag(cpu_flag::OVERFLOW, false);
            }
            5 => {
                // SUB
                let (result, carry) = dst.overflowing_sub(imm);
                let overflow = ((dst ^ imm) & (dst ^ result) & 0x8000) != 0;
                self.write_rm16(mode, rm, addr, result, memory_bus);
                self.set_flags_16(result);
                self.set_flag(cpu_flag::CARRY, carry);
                self.set_flag(cpu_flag::OVERFLOW, overflow);
            }
            6 => {
                // XOR
                let result = dst ^ imm;
                self.write_rm16(mode, rm, addr, result, memory_bus);
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
}
