use crate::{
    bus::Bus,
    cpu::{Cpu, cpu_flag, timing},
};

impl Cpu {
    /// CMP immediate to accumulator (opcodes 3C-3D)
    /// 3C: CMP AL, imm8
    /// 3D: CMP AX, imm16
    pub(in crate::cpu) fn cmp_imm_acc(&mut self, opcode: u8, bus: &Bus) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            // CMP AX, imm16
            let imm = self.fetch_word(bus);
            let (result, carry) = self.ax.overflowing_sub(imm);
            let overflow = ((self.ax ^ imm) & (self.ax ^ result) & 0x8000) != 0;

            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        } else {
            // CMP AL, imm8
            let imm = self.fetch_byte(bus);
            let al = (self.ax & 0xFF) as u8;
            let (result, carry) = al.overflowing_sub(imm);
            let overflow = ((al ^ imm) & (al ^ result) & 0x80) != 0;
            let aux_carry = (al & 0x0F) < (imm & 0x0F);

            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
            self.set_flag(cpu_flag::AUXILIARY, aux_carry);
        }

        // CMP immediate with accumulator: 4 cycles
        self.last_instruction_cycles = timing::cycles::CMP_IMM_ACC;
    }

    /// CMP r/m and register (opcodes 38-3B)
    /// 38: CMP r/m8, r8
    /// 39: CMP r/m16, r16
    /// 3A: CMP r8, r/m8
    /// 3B: CMP r16, r/m16
    pub(in crate::cpu) fn cmp_rm_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if is_word {
            // 16-bit cmp
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

            self.set_flags_16(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        } else {
            // 8-bit cmp
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

            self.set_flags_8(result);
            self.set_flag(cpu_flag::CARRY, carry);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
            self.set_flag(cpu_flag::AUXILIARY, aux_carry);
        }

        // Set cycle count
        self.last_instruction_cycles = if mode == 0b11 {
            // CMP reg, reg: 3 cycles
            timing::cycles::CMP_REG_REG
        } else if dir {
            // CMP reg, mem: 9 + EA cycles
            timing::cycles::CMP_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        } else {
            // CMP mem, reg: 9 + EA cycles
            timing::cycles::CMP_MEM_REG
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }
}
