use super::super::{Cpu, cpu_flag, timing};
use crate::Bus;

impl Cpu {
    

    /// IMUL - Signed Multiply with Immediate (opcode 0x69)
    /// IMUL r16, r/m16, imm16
    /// Multiplies r/m16 by imm16 and stores result in r16
    pub(in crate::cpu) fn imul_imm16(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);
        let src = self.read_rm16(mode, rm, addr, bus) as i16;
        let imm = self.fetch_word(bus) as i16;

        // Perform signed multiplication
        let result = (src as i32) * (imm as i32);

        // Store the lower 16 bits in the destination register
        self.set_reg16(reg, result as u16);

        // CF and OF are set if the result doesn't fit in a signed 16-bit value
        // i.e., if sign-extending the lower 16 bits doesn't equal the full result
        let sign_extended = ((result as u16) as i16) as i32;
        let overflow = sign_extended != result;
        self.set_flag(cpu_flag::CARRY, overflow);
        self.set_flag(cpu_flag::OVERFLOW, overflow);
        // Other flags (SF, ZF, PF, AF) are undefined per Intel spec

        // IMUL with immediate (80186+): 24 cycles (reg), 27+EA (mem)
        self.last_instruction_cycles = if mode == 0b11 {
            timing::cycles::IMUL_IMM_REG
        } else {
            timing::cycles::IMUL_IMM_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// IMUL - Signed Multiply with Immediate (opcode 0x6B)
    /// IMUL r16, r/m16, imm8 (sign-extended)
    /// Multiplies r/m16 by sign-extended imm8 and stores result in r16
    pub(in crate::cpu) fn imul_imm8(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);
        let src = self.read_rm16(mode, rm, addr, bus) as i16;
        let imm = self.fetch_byte(bus) as i8 as i16; // sign-extend 8→16

        let result = (src as i32) * (imm as i32);

        self.set_reg16(reg, result as u16);

        let sign_extended = ((result as u16) as i16) as i32;
        let overflow = sign_extended != result;
        self.set_flag(cpu_flag::CARRY, overflow);
        self.set_flag(cpu_flag::OVERFLOW, overflow);

        self.last_instruction_cycles = if mode == 0b11 {
            timing::cycles::IMUL_IMM_REG
        } else {
            timing::cycles::IMUL_IMM_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }
}
