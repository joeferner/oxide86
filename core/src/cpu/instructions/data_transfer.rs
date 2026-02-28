use super::super::{Cpu, cpu_flag, timing};
use crate::Bus;

impl Cpu {
    /// MOV immediate to register (opcodes B0-BF)
    /// B0-B7: MOV reg8, imm8
    /// B8-BF: MOV reg16, imm16
    pub(in crate::cpu) fn mov_imm_to_reg(&mut self, opcode: u8, bus: &Bus) {
        let reg = opcode & 0x07;
        let is_word = opcode & 0x08 != 0;

        if is_word {
            // 16-bit register
            let value = self.fetch_word(bus);
            self.set_reg16(reg, value);
        } else {
            // 8-bit register
            let value = self.fetch_byte(bus);
            self.set_reg8(reg, value);
        }

        // MOV immediate to register: 4 cycles
        self.last_instruction_cycles = timing::cycles::MOV_IMM_REG;
    }

    /// MOV accumulator to/from direct bus offset (opcodes A0-A3)
    /// A0: MOV AL, [moffs8] - Move byte at direct address to AL
    /// A1: MOV AX, [moffs16] - Move word at direct address to AX
    /// A2: MOV [moffs8], AL - Move AL to byte at direct address
    /// A3: MOV [moffs16], AX - Move AX to word at direct address
    pub(in crate::cpu) fn mov_acc_moffs(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let to_acc = opcode & 0x02 == 0; // 0 = to accumulator, 1 = from accumulator

        // Fetch the direct bus offset (16-bit address)
        let offset = self.fetch_word(bus);
        // Use segment override if present, otherwise use DS
        let segment = self.segment_override.unwrap_or(self.ds);
        let addr = Self::physical_address(segment, offset);

        if is_word {
            if to_acc {
                // MOV AX, [offset]
                self.ax = bus.read_u16(addr);
            } else {
                // MOV [offset], AX
                bus.write_u16(addr, self.ax);
            }
        } else if to_acc {
            // MOV AL, [offset]
            let value = bus.read_u8(addr);
            self.ax = (self.ax & 0xFF00) | (value as u16);
        } else {
            // MOV [offset], AL
            let value = (self.ax & 0xFF) as u8;
            bus.write_u8(addr, value);
        }

        // MOV acc, [addr] or [addr], acc: 10 cycles (direct addressing)
        self.last_instruction_cycles = if to_acc {
            timing::cycles::MOV_MEM_ACC
        } else {
            timing::cycles::MOV_ACC_MEM
        };
    }

    /// MOV r/m16 to segment register (opcode 8E)
    /// 8E: MOV segreg, r/m16
    /// Copies a 16-bit register or bus value to a segment register (ES, CS, SS, DS)
    /// Note: MOV to CS is not recommended as it affects instruction fetching
    pub(in crate::cpu) fn mov_rm_to_segreg(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, seg_reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // The reg field specifies which segment register (ES=0, CS=1, SS=2, DS=3)
        let value = self.read_rm16(mode, rm, addr, bus);
        self.set_segreg(seg_reg, value);

        // Calculate cycle timing
        self.last_instruction_cycles = if mode == 0b11 {
            // MOV segreg, reg: 2 cycles
            timing::cycles::MOV_RM_SEGREG_REG
        } else {
            // MOV segreg, mem: 8 + EA cycles
            timing::cycles::MOV_RM_SEGREG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// POP r/m16 (opcode 8F) - Group 1A
    /// 8F /0: POP r/m16
    /// Pops a word from stack to register or bus location
    pub(in crate::cpu) fn pop_rm16(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg_field, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // The reg field should be 0 for POP (it's an opcode extension)
        if reg_field != 0 {
            panic!(
                "Invalid opcode extension for 8F: expected /0, got /{}",
                reg_field
            );
        }

        let value = self.pop(bus);
        self.write_rm16(mode, rm, addr, value, bus);

        // Calculate cycle timing
        self.last_instruction_cycles = if mode == 0b11 {
            // POP reg: 8 cycles
            timing::cycles::POP_REG
        } else {
            // POP mem: 17 + EA cycles
            timing::cycles::POP_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// POPA - Pop All General Registers (opcode 0x61)
    /// Pops DI, SI, BP, (discard), BX, DX, CX, AX from stack
    /// 80186+ instruction
    pub(in crate::cpu) fn popa(&mut self, bus: &Bus) {
        self.di = self.pop(bus);
        self.si = self.pop(bus);
        self.bp = self.pop(bus);
        let _discard = self.pop(bus); // SP is discarded
        self.bx = self.pop(bus);
        self.dx = self.pop(bus);
        self.cx = self.pop(bus);
        self.ax = self.pop(bus);

        // POPA: 51 cycles (80186+)
        self.last_instruction_cycles = timing::cycles::POPA;
    }

    /// BOUND - Check Array Index Against Bounds (opcode 0x62)
    /// Checks if a signed register value is within bounds stored in bus
    /// If index < lower_bound or index > upper_bound, triggers INT 5
    /// 80186+ instruction
    pub(in crate::cpu) fn bound(&mut self, bus: &mut Bus) -> bool {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, _rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // BOUND only works with bus operands
        if mode == 0b11 {
            panic!("BOUND cannot use register operand");
        }

        // Get the index value from register (signed)
        let index = self.get_reg16(reg) as i16;

        // Read lower and upper bounds from bus (two consecutive signed words)
        let lower_bound = bus.read_u16(addr) as i16;
        let upper_bound = bus.read_u16(addr + 2) as i16;

        // Check if index is out of bounds
        if index < lower_bound || index > upper_bound {
            // Out of bounds - caller should trigger INT 5
            self.last_instruction_cycles = timing::cycles::BOUND_OUT; // 48-51 cycles
            return true;
        }

        // Within bounds - no exception
        self.last_instruction_cycles = timing::cycles::BOUND_IN; // 33-35 cycles
        false
    }
}
