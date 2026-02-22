use super::super::{Cpu, timing};
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

    /// MOV register to/from r/m (opcodes 88-8B)
    /// 88: MOV r/m8, r8
    /// 89: MOV r/m16, r16
    /// 8A: MOV r8, r/m8
    /// 8B: MOV r16, r/m16
    pub(in crate::cpu) fn mov_reg_rm(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if is_word {
            // 16-bit move
            if dir {
                // MOV reg16, r/m16
                let value = self.read_rm16(mode, rm, addr, bus);
                self.set_reg16(reg, value);
            } else {
                // MOV r/m16, reg16
                let value = self.get_reg16(reg);
                self.write_rm16(mode, rm, addr, value, bus);
            }
        } else {
            // 8-bit move
            if dir {
                // MOV reg8, r/m8
                let value = self.read_rm8(mode, rm, addr, bus);
                self.set_reg8(reg, value);
            } else {
                // MOV r/m8, reg8
                let value = self.get_reg8(reg);
                self.write_rm8(mode, rm, addr, value, bus);
            }
        }

        // Calculate cycle timing based on operands
        self.last_instruction_cycles = if mode == 0b11 {
            // MOV reg, reg: 2 cycles
            timing::cycles::MOV_REG_REG
        } else if dir {
            // MOV reg, mem: 8 + EA cycles
            timing::cycles::MOV_MEM_REG
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        } else {
            // MOV mem, reg: 9 + EA cycles
            timing::cycles::MOV_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// PUSH 16-bit register (opcodes 50-57)
    /// Push register onto stack
    /// 8086 PUSH SP behavior: pushes SP-2 (value after decrement)
    /// 80286+ PUSH SP behavior: pushes original SP value
    pub(in crate::cpu) fn push_reg16(&mut self, opcode: u8, bus: &mut Bus) {
        let reg = opcode & 0x07;
        if reg == 4 && self.cpu_type == crate::CpuType::I8086 {
            // PUSH SP on 8086: push the decremented value (post-decrement SP)
            self.sp = self.sp.wrapping_sub(2);
            let value = self.sp;
            let addr = Self::physical_address(self.ss, self.sp);
            bus.write_u16(addr, value);
        } else {
            let value = self.get_reg16(reg);
            self.push(value, bus);
        }

        // PUSH register: 11 cycles
        self.last_instruction_cycles = timing::cycles::PUSH_REG;
    }

    /// POP 16-bit register (opcodes 58-5F)
    /// Pop from stack to register
    pub(in crate::cpu) fn pop_reg16(&mut self, opcode: u8, bus: &mut Bus) {
        let reg = opcode & 0x07;
        let value = self.pop(bus);
        self.set_reg16(reg, value);

        // POP register: 8 cycles
        self.last_instruction_cycles = timing::cycles::POP_REG;
    }

    /// PUSH immediate (opcode 68: 16-bit, 6A: sign-extended 8-bit)
    pub(in crate::cpu) fn push_imm(&mut self, opcode: u8, bus: &mut Bus) {
        let value = if opcode == 0x68 {
            // PUSH imm16
            self.fetch_word(bus)
        } else {
            // PUSH imm8 (sign-extended to 16 bits)
            let imm8 = self.fetch_byte(bus);
            if imm8 & 0x80 != 0 {
                0xFF00 | (imm8 as u16)
            } else {
                imm8 as u16
            }
        };
        self.push(value, bus);

        // PUSH immediate: 10 cycles (80186+)
        self.last_instruction_cycles = timing::cycles::PUSH_IMM;
    }

    /// MOV immediate to r/m (opcodes C6-C7)
    /// C6: MOV r/m8, imm8
    /// C7: MOV r/m16, imm16
    pub(in crate::cpu) fn mov_imm_to_rm(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let modrm = self.fetch_byte(bus);
        let (mode, _reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // The reg field should be 0 for MOV immediate
        // (it's part of the opcode extension)

        if is_word {
            // MOV r/m16, imm16
            let value = self.fetch_word(bus);
            self.write_rm16(mode, rm, addr, value, bus);
        } else {
            // MOV r/m8, imm8
            let value = self.fetch_byte(bus);
            self.write_rm8(mode, rm, addr, value, bus);
        }

        // Calculate cycle timing
        self.last_instruction_cycles = if mode == 0b11 {
            // MOV reg, imm: 4 cycles
            timing::cycles::MOV_IMM_REG
        } else {
            // MOV mem, imm: 10 + EA cycles
            timing::cycles::MOV_IMM_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
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

    /// MOV segment register to r/m16 (opcode 8C)
    /// 8C: MOV r/m16, segreg
    /// Copies a segment register (ES, CS, SS, DS) to a 16-bit register or bus location
    pub(in crate::cpu) fn mov_segreg_to_rm(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, seg_reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // The reg field specifies which segment register (ES=0, CS=1, SS=2, DS=3)
        let value = self.get_segreg(seg_reg);
        self.write_rm16(mode, rm, addr, value, bus);

        // Calculate cycle timing
        self.last_instruction_cycles = if mode == 0b11 {
            // MOV reg, segreg: 2 cycles
            timing::cycles::MOV_SEGREG_RM_REG
        } else {
            // MOV mem, segreg: 9 + EA cycles
            timing::cycles::MOV_SEGREG_RM_MEM
                + timing::calculate_ea_cycles(mode, seg_reg, self.segment_override.is_some())
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

    /// PUSH segment register (opcodes 06, 0E, 16, 1E)
    /// 06: PUSH ES
    /// 0E: PUSH CS
    /// 16: PUSH SS
    /// 1E: PUSH DS
    pub(in crate::cpu) fn push_segreg(&mut self, opcode: u8, bus: &mut Bus) {
        let seg = match opcode {
            0x06 => 0, // ES
            0x0E => 1, // CS
            0x16 => 2, // SS
            0x1E => 3, // DS
            _ => unreachable!(),
        };
        let value = self.get_segreg(seg);
        self.push(value, bus);

        // PUSH segment register: 10 cycles
        self.last_instruction_cycles = timing::cycles::PUSH_SEGREG;
    }

    /// POP segment register (opcodes 07, 0F, 17, 1F)
    /// 07: POP ES
    /// 0F: POP CS (note: POP CS is unusual, typically not used)
    /// 17: POP SS
    /// 1F: POP DS
    pub(in crate::cpu) fn pop_segreg(&mut self, opcode: u8, bus: &mut Bus) {
        let seg = match opcode {
            0x07 => 0, // ES
            0x0F => 1, // CS
            0x17 => 2, // SS
            0x1F => 3, // DS
            _ => unreachable!(),
        };
        let value = self.pop(bus);
        self.set_segreg(seg, value);

        // POP segment register: 8 cycles
        self.last_instruction_cycles = timing::cycles::POP_SEGREG;
    }

    /// PUSHF - Push Flags Register (opcode 9C)
    /// Pushes the FLAGS register onto the stack
    pub(in crate::cpu) fn pushf(&mut self, bus: &mut Bus) {
        self.push(self.flags, bus);

        // PUSHF: 10 cycles
        self.last_instruction_cycles = timing::cycles::PUSHF;
    }

    /// POPF - Pop Flags Register (opcode 9D)
    /// Pops a word from the stack into the FLAGS register
    /// On 8086: only bits 0-11 can be modified, bit 1 is always 1
    pub(in crate::cpu) fn popf(&mut self, bus: &mut Bus) {
        let value = self.pop(bus);
        // 8086 behavior: only allow bits 0-11 to be modified, force bit 1 to 1
        self.flags = (value & 0x0FFF) | 0x0002;

        // POPF: 8 cycles
        self.last_instruction_cycles = timing::cycles::POPF;
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

    /// PUSH r/m16 (opcode FF /6) - Group 5
    /// FF /6: PUSH r/m16
    /// Pushes a word from register or bus location onto stack
    pub(in crate::cpu) fn push_rm16(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg_field, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // The reg field should be 6 for PUSH (it's an opcode extension)
        if reg_field != 6 {
            panic!(
                "Invalid opcode extension for FF /6: expected /6, got /{}",
                reg_field
            );
        }

        let value = self.read_rm16(mode, rm, addr, bus);
        self.push(value, bus);

        // Calculate cycle timing
        self.last_instruction_cycles = if mode == 0b11 {
            // PUSH reg: 11 cycles
            timing::cycles::PUSH_REG
        } else {
            // PUSH mem: 16 + EA cycles
            timing::cycles::PUSH_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// XCHG register with accumulator (opcodes 90-97)
    /// 90: NOP (XCHG AX, AX) - special case
    /// 91-97: XCHG AX, reg16
    pub(in crate::cpu) fn xchg_ax_reg(&mut self, opcode: u8) {
        let reg = opcode & 0x07;
        if reg == 0 {
            // NOP - XCHG AX, AX does nothing
            self.last_instruction_cycles = timing::cycles::NOP;
            return;
        }
        let temp = self.ax;
        self.ax = self.get_reg16(reg);
        self.set_reg16(reg, temp);

        // XCHG AX, reg: 3 cycles
        self.last_instruction_cycles = timing::cycles::XCHG_REG_ACC;
    }

    /// XCHG register/bus with register (opcodes 86-87)
    /// 86: XCHG r/m8, r8
    /// 87: XCHG r/m16, r16
    pub(in crate::cpu) fn xchg_rm_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if is_word {
            // 16-bit exchange
            let reg_val = self.get_reg16(reg);
            let rm_val = self.read_rm16(mode, rm, addr, bus);
            self.set_reg16(reg, rm_val);
            self.write_rm16(mode, rm, addr, reg_val, bus);
        } else {
            // 8-bit exchange
            let reg_val = self.get_reg8(reg);
            let rm_val = self.read_rm8(mode, rm, addr, bus);
            self.set_reg8(reg, rm_val);
            self.write_rm8(mode, rm, addr, reg_val, bus);
        }

        // Calculate cycle timing
        self.last_instruction_cycles = if mode == 0b11 {
            // XCHG reg, reg: 4 cycles
            timing::cycles::XCHG_REG_REG
        } else {
            // XCHG reg, mem: 17 + EA cycles
            timing::cycles::XCHG_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }

    /// XLAT - Table Look-up Translation (opcode 0xD7)
    /// Translates AL using lookup table at DS:BX
    /// AL = [DS:BX + AL]
    pub(in crate::cpu) fn xlat(&mut self, bus: &Bus) {
        let al = (self.ax & 0xFF) as u8;
        let offset = self.bx.wrapping_add(al as u16);
        // Use segment override if present, otherwise use DS
        let segment = self.segment_override.unwrap_or(self.ds);
        let addr = Self::physical_address(segment, offset);
        let value = bus.read_u8(addr);
        self.ax = (self.ax & 0xFF00) | (value as u16);

        // XLAT: 11 cycles
        self.last_instruction_cycles = timing::cycles::XLAT;
    }

    /// LEA - Load Effective Address (opcode 0x8D)
    /// Loads the offset of the source operand into destination register
    pub(in crate::cpu) fn lea(&mut self, bus: &Bus) {
        let modrm = self.fetch_byte(bus);
        let mode = modrm >> 6;
        let reg = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;

        // LEA only works with bus operands (mode != 11)
        if mode == 0b11 {
            panic!("LEA cannot use register operand");
        }

        // Calculate the effective address offset (not physical address)
        let offset = match rm {
            0b000 => self.bx.wrapping_add(self.si), // [BX + SI]
            0b001 => self.bx.wrapping_add(self.di), // [BX + DI]
            0b010 => self.bp.wrapping_add(self.si), // [BP + SI]
            0b011 => self.bp.wrapping_add(self.di), // [BP + DI]
            0b100 => self.si,                       // [SI]
            0b101 => self.di,                       // [DI]
            0b110 => {
                if mode == 0b00 {
                    // Special case: direct address
                    self.fetch_word(bus)
                } else {
                    self.bp // [BP]
                }
            }
            0b111 => self.bx, // [BX]
            _ => unreachable!(),
        };

        // Add displacement based on mode
        let effective_offset = match mode {
            0b00 => offset, // No displacement (except for direct addressing handled above)
            0b01 => {
                // 8-bit signed displacement
                let disp = self.fetch_byte(bus) as i8;
                offset.wrapping_add(disp as i16 as u16)
            }
            0b10 => {
                // 16-bit displacement
                let disp = self.fetch_word(bus);
                offset.wrapping_add(disp)
            }
            _ => unreachable!(),
        };

        self.set_reg16(reg, effective_offset);

        // LEA: 2 + EA cycles (EA calculation is done even though bus isn't accessed)
        self.last_instruction_cycles = timing::cycles::LEA
            + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some());
    }

    /// LDS - Load Pointer using DS (opcode 0xC5)
    /// Loads far pointer from bus into register and DS
    pub(in crate::cpu) fn lds(&mut self, bus: &Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, _rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // LDS only works with bus operands
        if mode == 0b11 {
            panic!("LDS cannot use register operand");
        }

        // Read offset and segment from bus (4 bytes total)
        let offset = bus.read_u16(addr);
        let segment = bus.read_u16(addr + 2);

        self.set_reg16(reg, offset);
        self.ds = segment;

        // LDS: 16 + EA cycles
        let rm = modrm & 0x07;
        self.last_instruction_cycles = timing::cycles::LDS
            + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some());
    }

    /// LES - Load Pointer using ES (opcode 0xC4)
    /// Loads far pointer from bus into register and ES
    pub(in crate::cpu) fn les(&mut self, bus: &Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, _rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // LES only works with bus operands
        if mode == 0b11 {
            panic!("LES cannot use register operand");
        }

        // Read offset and segment from bus (4 bytes total)
        let offset = bus.read_u16(addr);
        let segment = bus.read_u16(addr + 2);

        self.set_reg16(reg, offset);
        self.es = segment;

        // LES: 16 + EA cycles
        let rm = modrm & 0x07;
        self.last_instruction_cycles = timing::cycles::LES
            + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some());
    }

    /// LAHF - Load AH from Flags (opcode 0x9F)
    /// Loads SF, ZF, AF, PF, CF into AH
    pub(in crate::cpu) fn lahf(&mut self) {
        let ah = (self.flags & 0xFF) as u8;
        self.ax = (self.ax & 0x00FF) | ((ah as u16) << 8);

        // LAHF: 4 cycles
        self.last_instruction_cycles = timing::cycles::LAHF;
    }

    /// SAHF - Store AH into Flags (opcode 0x9E)
    /// Stores AH into SF, ZF, AF, PF, CF
    pub(in crate::cpu) fn sahf(&mut self) {
        let ah = ((self.ax >> 8) & 0xFF) as u8;
        // Only update lower 8 bits of flags (SF, ZF, 0, AF, 0, PF, 1, CF)
        // Preserve upper 8 bits
        self.flags = (self.flags & 0xFF00) | (ah as u16);

        // SAHF: 4 cycles
        self.last_instruction_cycles = timing::cycles::SAHF;
    }

    /// PUSHA - Push All General Registers (opcode 0x60)
    /// Pushes AX, CX, DX, BX, original SP, BP, SI, DI onto the stack
    /// 80186+ instruction
    pub(in crate::cpu) fn pusha(&mut self, bus: &mut Bus) {
        let original_sp = self.sp;
        self.push(self.ax, bus);
        self.push(self.cx, bus);
        self.push(self.dx, bus);
        self.push(self.bx, bus);
        self.push(original_sp, bus);
        self.push(self.bp, bus);
        self.push(self.si, bus);
        self.push(self.di, bus);

        // PUSHA: 36 cycles (80186+)
        self.last_instruction_cycles = timing::cycles::PUSHA;
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
