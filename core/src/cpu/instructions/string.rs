use crate::{
    bus::Bus,
    cpu::{Cpu, cpu_flag, instructions::RepeatPrefix, timing},
};

impl Cpu {
    /// LODS - Load String (opcodes AC-AD)
    /// AC: LODSB - Load byte from DS:SI into AL
    /// AD: LODSW - Load word from DS:SI into AX
    ///
    /// Loads data from DS:SI into AL/AX, then increments/decrements SI based on DF.
    /// Note: Segment override can apply to DS:SI
    pub(in crate::cpu) fn lods(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;

        // Handle repeat prefix
        if self.repeat_prefix.is_some() {
            let count = self.cx;
            while self.cx != 0 {
                self.lods_once(is_word, bus);
                self.cx = self.cx.wrapping_sub(1);
            }
            // REP LODS: 9 + 13*CX cycles
            bus.increment_cycle_count(
                timing::cycles::REP_LODS_BASE + (timing::cycles::REP_LODS_PER_ITER * count as u32),
            );
        } else {
            self.lods_once(is_word, bus);
            // LODS (no REP): 12 cycles
            bus.increment_cycle_count(timing::cycles::LODS)
        }
    }

    fn lods_once(&mut self, is_word: bool, bus: &Bus) {
        if is_word {
            // LODSW - Load word
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let addr = bus.physical_address(src_seg, self.si);
            self.ax = bus.memory_read_u16(addr);

            // Update SI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.si = self.si.wrapping_sub(2);
            } else {
                self.si = self.si.wrapping_add(2);
            }
        } else {
            // LODSB - Load byte
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let addr = bus.physical_address(src_seg, self.si);
            let value = bus.memory_read_u8(addr);
            self.ax = (self.ax & 0xFF00) | (value as u16);

            // Update SI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.si = self.si.wrapping_sub(1);
            } else {
                self.si = self.si.wrapping_add(1);
            }
        }
    }

    /// CLD - Clear Direction Flag (opcode FC)
    /// Sets DF to 0, causing string operations to increment SI/DI (forward direction)
    pub(in crate::cpu) fn cld(&mut self, bus: &mut Bus) {
        self.set_flag(cpu_flag::DIRECTION, false);

        // CLD: 2 cycles
        bus.increment_cycle_count(timing::cycles::FLAG_OPS)
    }

    /// STOS - Store String (opcodes AA-AB)
    /// AA: STOSB - Store AL into byte at ES:DI
    /// AB: STOSW - Store AX into word at ES:DI
    ///
    /// Stores AL/AX into ES:DI, then increments/decrements DI based on DF.
    pub(in crate::cpu) fn stos(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;

        // Handle repeat prefix
        if self.repeat_prefix.is_some() {
            let count = self.cx;
            while self.cx != 0 {
                self.stos_once(is_word, bus);
                self.cx = self.cx.wrapping_sub(1);
            }
            // REP STOS: 9 + 10*CX cycles
            bus.increment_cycle_count(
                timing::cycles::REP_STOS_BASE + (timing::cycles::REP_STOS_PER_ITER * count as u32),
            );
        } else {
            self.stos_once(is_word, bus);
            // STOS (no REP): 11 cycles
            bus.increment_cycle_count(timing::cycles::STOS)
        }
    }

    fn stos_once(&mut self, is_word: bool, bus: &mut Bus) {
        if is_word {
            // STOSW - Store word
            let addr = bus.physical_address(self.es, self.di);
            bus.memory_write_u16(addr, self.ax);

            // Update DI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.di = self.di.wrapping_sub(2);
            } else {
                self.di = self.di.wrapping_add(2);
            }
        } else {
            // STOSB - Store byte
            let addr = bus.physical_address(self.es, self.di);
            let al = (self.ax & 0xFF) as u8;
            bus.memory_write_u8(addr, al);

            // Update DI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.di = self.di.wrapping_sub(1);
            } else {
                self.di = self.di.wrapping_add(1);
            }
        }
    }

    /// MOVS - Move String (opcodes A4-A5)
    /// A4: MOVSB - Move byte from DS:SI to ES:DI
    /// A5: MOVSW - Move word from DS:SI to ES:DI
    ///
    /// Moves data from DS:SI to ES:DI, then increments/decrements SI and DI
    /// based on the Direction Flag (DF).
    /// If DF=0: increment (forward), if DF=1: decrement (backward)
    /// Note: Segment override can apply to source (DS:SI) but not destination (ES:DI is hardcoded)
    pub(in crate::cpu) fn movs(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;

        // Handle repeat prefix
        if self.repeat_prefix.is_some() {
            let count = self.cx;
            while self.cx != 0 {
                self.movs_once(is_word, bus);
                self.cx = self.cx.wrapping_sub(1);
            }
            // REP MOVS: 9 + 17*CX cycles
            bus.increment_cycle_count(
                timing::cycles::REP_MOVS_BASE + (timing::cycles::REP_MOVS_PER_ITER * count as u32),
            );
        } else {
            self.movs_once(is_word, bus);
            // MOVS (no REP): 18 cycles
            bus.increment_cycle_count(timing::cycles::MOVS)
        }
    }

    fn movs_once(&mut self, is_word: bool, bus: &mut Bus) {
        if is_word {
            // MOVSW - Move word
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let src_addr = bus.physical_address(src_seg, self.si);
            let dst_addr = bus.physical_address(self.es, self.di); // ES:DI is always ES
            let value = bus.memory_read_u16(src_addr);
            bus.memory_write_u16(dst_addr, value);

            // Update SI and DI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                // DF=1: decrement
                self.si = self.si.wrapping_sub(2);
                self.di = self.di.wrapping_sub(2);
            } else {
                // DF=0: increment
                self.si = self.si.wrapping_add(2);
                self.di = self.di.wrapping_add(2);
            }
        } else {
            // MOVSB - Move byte
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let src_addr = bus.physical_address(src_seg, self.si);
            let dst_addr = bus.physical_address(self.es, self.di); // ES:DI is always ES
            let value = bus.memory_read_u8(src_addr);
            bus.memory_write_u8(dst_addr, value);

            // Update SI and DI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                // DF=1: decrement
                self.si = self.si.wrapping_sub(1);
                self.di = self.di.wrapping_sub(1);
            } else {
                // DF=0: increment
                self.si = self.si.wrapping_add(1);
                self.di = self.di.wrapping_add(1);
            }
        }
    }

    /// CMPS - Compare String (opcodes A6-A7)
    /// A6: CMPSB - Compare byte at DS:SI with byte at ES:DI
    /// A7: CMPSW - Compare word at DS:SI with word at ES:DI
    ///
    /// Compares DS:SI with ES:DI (subtracts ES:DI from DS:SI), sets flags,
    /// then increments/decrements SI and DI based on DF.
    /// Does not store the result, only affects flags.
    /// Note: Segment override can apply to source (DS:SI) but not destination (ES:DI is hardcoded)
    pub(in crate::cpu) fn cmps(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;

        // Handle repeat prefix
        match self.repeat_prefix {
            Some(RepeatPrefix::Rep) | Some(RepeatPrefix::Repe) => {
                // REPE/REPZ: Repeat while CX != 0 and ZF = 1
                let start_cx = self.cx;
                while self.cx != 0 {
                    self.cmps_once(is_word, bus);
                    self.cx = self.cx.wrapping_sub(1);
                    if !self.get_flag(cpu_flag::ZERO) {
                        break; // Stop if not equal (ZF=0)
                    }
                }
                // Calculate actual iterations performed
                let iterations = start_cx - self.cx;
                // REP CMPS: 9 + 22*count cycles
                bus.increment_cycle_count(
                    timing::cycles::REP_CMPS_BASE
                        + (timing::cycles::REP_CMPS_PER_ITER * iterations as u32),
                );
            }
            Some(RepeatPrefix::Repne) => {
                // REPNE/REPNZ: Repeat while CX != 0 and ZF = 0
                let start_cx = self.cx;
                while self.cx != 0 {
                    self.cmps_once(is_word, bus);
                    self.cx = self.cx.wrapping_sub(1);
                    if self.get_flag(cpu_flag::ZERO) {
                        break; // Stop if equal (ZF=1)
                    }
                }
                // Calculate actual iterations performed
                let iterations = start_cx - self.cx;
                // REP CMPS: 9 + 22*count cycles
                bus.increment_cycle_count(
                    timing::cycles::REP_CMPS_BASE
                        + (timing::cycles::REP_CMPS_PER_ITER * iterations as u32),
                );
            }
            None => {
                self.cmps_once(is_word, bus);
                // CMPS (no REP): 22 cycles
                bus.increment_cycle_count(timing::cycles::CMPS);
            }
        }
    }

    fn cmps_once(&mut self, is_word: bool, bus: &Bus) {
        if is_word {
            // CMPSW - Compare word
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let src_addr = bus.physical_address(src_seg, self.si);
            let dst_addr = bus.physical_address(self.es, self.di); // ES:DI is always ES
            let src = bus.memory_read_u16(src_addr);
            let dst = bus.memory_read_u16(dst_addr);

            // Perform subtraction to set flags (src - dst)
            let result = src.wrapping_sub(dst);
            self.set_flags_sub_16(src, dst, result);

            // Update SI and DI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.si = self.si.wrapping_sub(2);
                self.di = self.di.wrapping_sub(2);
            } else {
                self.si = self.si.wrapping_add(2);
                self.di = self.di.wrapping_add(2);
            }
        } else {
            // CMPSB - Compare byte
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let src_addr = bus.physical_address(src_seg, self.si);
            let dst_addr = bus.physical_address(self.es, self.di); // ES:DI is always ES
            let src = bus.memory_read_u8(src_addr);
            let dst = bus.memory_read_u8(dst_addr);

            // Perform subtraction to set flags (src - dst)
            let result = src.wrapping_sub(dst);
            self.set_flags_sub_8(src, dst, result);

            // Update SI and DI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.si = self.si.wrapping_sub(1);
                self.di = self.di.wrapping_sub(1);
            } else {
                self.si = self.si.wrapping_add(1);
                self.di = self.di.wrapping_add(1);
            }
        }
    }

    /// SCAS - Scan String (opcodes AE-AF)
    /// AE: SCASB - Compare AL with byte at ES:DI
    /// AF: SCASW - Compare AX with word at ES:DI
    ///
    /// Compares AL/AX with ES:DI (subtracts ES:DI from AL/AX), sets flags,
    /// then increments/decrements DI based on DF.
    pub(in crate::cpu) fn scas(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;

        // Handle repeat prefix
        match self.repeat_prefix {
            Some(RepeatPrefix::Rep) | Some(RepeatPrefix::Repe) => {
                // REPE/REPZ: Repeat while CX != 0 and ZF = 1
                let start_cx = self.cx;
                while self.cx != 0 {
                    self.scas_once(is_word, bus);
                    self.cx = self.cx.wrapping_sub(1);
                    if !self.get_flag(cpu_flag::ZERO) {
                        break; // Stop if not equal (ZF=0)
                    }
                }
                // Calculate actual iterations performed
                let iterations = start_cx - self.cx;
                // REP SCAS: 9 + 15*count cycles
                bus.increment_cycle_count(
                    timing::cycles::REP_SCAS_BASE
                        + (timing::cycles::REP_SCAS_PER_ITER * iterations as u32),
                );
            }
            Some(RepeatPrefix::Repne) => {
                // REPNE/REPNZ: Repeat while CX != 0 and ZF = 0
                let start_cx = self.cx;
                while self.cx != 0 {
                    self.scas_once(is_word, bus);
                    self.cx = self.cx.wrapping_sub(1);
                    if self.get_flag(cpu_flag::ZERO) {
                        break; // Stop if equal (ZF=1)
                    }
                }
                // Calculate actual iterations performed
                let iterations = start_cx - self.cx;
                // REP SCAS: 9 + 15*count cycles
                bus.increment_cycle_count(
                    timing::cycles::REP_SCAS_BASE
                        + (timing::cycles::REP_SCAS_PER_ITER * iterations as u32),
                );
            }
            None => {
                self.scas_once(is_word, bus);
                // SCAS (no REP): 15 cycles
                bus.increment_cycle_count(timing::cycles::SCAS)
            }
        }
    }

    fn scas_once(&mut self, is_word: bool, bus: &Bus) {
        if is_word {
            // SCASW - Scan word
            let addr = bus.physical_address(self.es, self.di);
            let value = bus.memory_read_u16(addr);

            // Compare AX with bus value (AX - value)
            let result = self.ax.wrapping_sub(value);
            self.set_flags_sub_16(self.ax, value, result);

            // Update DI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.di = self.di.wrapping_sub(2);
            } else {
                self.di = self.di.wrapping_add(2);
            }
        } else {
            // SCASB - Scan byte
            let addr = bus.physical_address(self.es, self.di);
            let value = bus.memory_read_u8(addr);
            let al = (self.ax & 0xFF) as u8;

            // Compare AL with bus value (AL - value)
            let result = al.wrapping_sub(value);
            self.set_flags_sub_8(al, value, result);

            // Update DI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.di = self.di.wrapping_sub(1);
            } else {
                self.di = self.di.wrapping_add(1);
            }
        }
    }

    /// Helper function to set flags for 8-bit subtraction
    /// Used by CMPS and SCAS
    fn set_flags_sub_8(&mut self, left: u8, right: u8, result: u8) {
        // Zero, Sign, Parity flags
        self.set_flag(cpu_flag::ZERO, result == 0);
        self.set_flag(cpu_flag::SIGN, (result & 0x80) != 0);
        self.set_flag(cpu_flag::PARITY, result.count_ones().is_multiple_of(2));

        // Carry flag (set if borrow occurred)
        self.set_flag(cpu_flag::CARRY, left < right);

        // Auxiliary carry (borrow from bit 3)
        let aux_carry = (left & 0x0F) < (right & 0x0F);
        self.set_flag(cpu_flag::AUXILIARY, aux_carry);

        // Overflow flag (signed overflow)
        let left_sign = (left & 0x80) != 0;
        let right_sign = (right & 0x80) != 0;
        let result_sign = (result & 0x80) != 0;
        let overflow = left_sign != right_sign && left_sign != result_sign;
        self.set_flag(cpu_flag::OVERFLOW, overflow);
    }

    /// Helper function to set flags for 16-bit subtraction
    /// Used by CMPS and SCAS
    fn set_flags_sub_16(&mut self, left: u16, right: u16, result: u16) {
        // Zero, Sign, Parity flags (parity on low byte only)
        self.set_flag(cpu_flag::ZERO, result == 0);
        self.set_flag(cpu_flag::SIGN, (result & 0x8000) != 0);
        self.set_flag(
            cpu_flag::PARITY,
            (result as u8).count_ones().is_multiple_of(2),
        );

        // Carry flag (set if borrow occurred)
        self.set_flag(cpu_flag::CARRY, left < right);

        // Auxiliary carry (borrow from bit 3)
        let aux_carry = (left & 0x0F) < (right & 0x0F);
        self.set_flag(cpu_flag::AUXILIARY, aux_carry);

        // Overflow flag (signed overflow)
        let left_sign = (left & 0x8000) != 0;
        let right_sign = (right & 0x8000) != 0;
        let result_sign = (result & 0x8000) != 0;
        let overflow = left_sign != right_sign && left_sign != result_sign;
        self.set_flag(cpu_flag::OVERFLOW, overflow);
    }

    /// STD - Set Direction Flag (opcode FD)
    /// Sets DF to 1, causing string operations to decrement SI/DI (backward direction)
    pub(in crate::cpu) fn std_flag(&mut self, bus: &mut Bus) {
        self.set_flag(cpu_flag::DIRECTION, true);

        // STD: 2 cycles
        bus.increment_cycle_count(timing::cycles::FLAG_OPS)
    }

    /// INS - Input String from Port (opcodes 6C-6D)
    /// 6C: INSB - Input byte from port DX to ES:DI
    /// 6D: INSW - Input word from port DX to ES:DI
    ///
    /// Reads data from I/O port DX into ES:DI, then increments/decrements DI based on DF.
    pub(in crate::cpu) fn ins(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;

        // Handle repeat prefix
        if self.repeat_prefix.is_some() {
            while self.cx != 0 {
                self.ins_once(is_word, bus);
                self.cx = self.cx.wrapping_sub(1);
            }
        } else {
            self.ins_once(is_word, bus);
        }
    }

    fn ins_once(&mut self, is_word: bool, bus: &mut Bus) {
        let port = self.dx;

        if is_word {
            // INSW - Input word; route ATA data port through the ATA handler
            let value = if port == 0x1F0 {
                // TODO bios.ata_read_u16()
                todo!("bios.ata_read_u16");
            } else {
                bus.io_read_u16(port)
            };
            let addr = bus.physical_address(self.es, self.di);
            bus.memory_write_u16(addr, value);

            // Update DI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.di = self.di.wrapping_sub(2);
            } else {
                self.di = self.di.wrapping_add(2);
            }
        } else {
            // INSB - Input byte
            let value = bus.io_read_u8(port);
            let addr = bus.physical_address(self.es, self.di);
            bus.memory_write_u8(addr, value);

            // Update DI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.di = self.di.wrapping_sub(1);
            } else {
                self.di = self.di.wrapping_add(1);
            }
        }
    }

    /// OUTS - Output String to Port (opcodes 6E-6F)
    /// 6E: OUTSB - Output byte from DS:SI to port DX
    /// 6F: OUTSW - Output word from DS:SI to port DX
    ///
    /// Writes data from DS:SI to I/O port DX, then increments/decrements SI based on DF.
    pub(in crate::cpu) fn outs(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;

        // Handle repeat prefix
        if self.repeat_prefix.is_some() {
            while self.cx != 0 {
                self.outs_once(is_word, bus);
                self.cx = self.cx.wrapping_sub(1);
            }
        } else {
            self.outs_once(is_word, bus);
        }
    }

    fn outs_once(&mut self, is_word: bool, bus: &mut Bus) {
        let port = self.dx;

        if is_word {
            // OUTSW - Output word
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let addr = bus.physical_address(src_seg, self.si);
            let value = bus.memory_read_u16(addr);
            bus.io_write_u16(port, value);

            // Update SI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.si = self.si.wrapping_sub(2);
            } else {
                self.si = self.si.wrapping_add(2);
            }
        } else {
            // OUTSB - Output byte
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let addr = bus.physical_address(src_seg, self.si);
            let value = bus.memory_read_u8(addr);
            bus.io_write_u8(port, value);

            // Update SI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.si = self.si.wrapping_sub(1);
            } else {
                self.si = self.si.wrapping_add(1);
            }
        }
    }
}
