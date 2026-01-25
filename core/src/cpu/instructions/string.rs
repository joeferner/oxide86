use super::super::Cpu;
use crate::memory::Memory;

// Flag constant from parent module
const FLAG_DIRECTION: u16 = 1 << 10;
const FLAG_ZERO: u16 = 1 << 6;
const FLAG_SIGN: u16 = 1 << 7;
const FLAG_PARITY: u16 = 1 << 2;
const FLAG_CARRY: u16 = 1 << 0;
const FLAG_AUXILIARY: u16 = 1 << 4;
const FLAG_OVERFLOW: u16 = 1 << 11;

impl Cpu {
    /// MOVS - Move String (opcodes A4-A5)
    /// A4: MOVSB - Move byte from DS:SI to ES:DI
    /// A5: MOVSW - Move word from DS:SI to ES:DI
    ///
    /// Moves data from DS:SI to ES:DI, then increments/decrements SI and DI
    /// based on the Direction Flag (DF).
    /// If DF=0: increment (forward), if DF=1: decrement (backward)
    pub(in crate::cpu) fn movs(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            // MOVSW - Move word
            let src_addr = Self::physical_address(self.ds, self.si);
            let dst_addr = Self::physical_address(self.es, self.di);
            let value = memory.read_word(src_addr);
            memory.write_word(dst_addr, value);

            // Update SI and DI based on direction flag
            if self.get_flag(FLAG_DIRECTION) {
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
            let src_addr = Self::physical_address(self.ds, self.si);
            let dst_addr = Self::physical_address(self.es, self.di);
            let value = memory.read_byte(src_addr);
            memory.write_byte(dst_addr, value);

            // Update SI and DI based on direction flag
            if self.get_flag(FLAG_DIRECTION) {
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
    pub(in crate::cpu) fn cmps(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            // CMPSW - Compare word
            let src_addr = Self::physical_address(self.ds, self.si);
            let dst_addr = Self::physical_address(self.es, self.di);
            let src = memory.read_word(src_addr);
            let dst = memory.read_word(dst_addr);

            // Perform subtraction to set flags (src - dst)
            let result = src.wrapping_sub(dst);
            self.set_flags_sub_16(src, dst, result);

            // Update SI and DI based on direction flag
            if self.get_flag(FLAG_DIRECTION) {
                self.si = self.si.wrapping_sub(2);
                self.di = self.di.wrapping_sub(2);
            } else {
                self.si = self.si.wrapping_add(2);
                self.di = self.di.wrapping_add(2);
            }
        } else {
            // CMPSB - Compare byte
            let src_addr = Self::physical_address(self.ds, self.si);
            let dst_addr = Self::physical_address(self.es, self.di);
            let src = memory.read_byte(src_addr);
            let dst = memory.read_byte(dst_addr);

            // Perform subtraction to set flags (src - dst)
            let result = src.wrapping_sub(dst);
            self.set_flags_sub_8(src, dst, result);

            // Update SI and DI based on direction flag
            if self.get_flag(FLAG_DIRECTION) {
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
    pub(in crate::cpu) fn scas(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            // SCASW - Scan word
            let addr = Self::physical_address(self.es, self.di);
            let value = memory.read_word(addr);

            // Compare AX with memory value (AX - value)
            let result = self.ax.wrapping_sub(value);
            self.set_flags_sub_16(self.ax, value, result);

            // Update DI based on direction flag
            if self.get_flag(FLAG_DIRECTION) {
                self.di = self.di.wrapping_sub(2);
            } else {
                self.di = self.di.wrapping_add(2);
            }
        } else {
            // SCASB - Scan byte
            let addr = Self::physical_address(self.es, self.di);
            let value = memory.read_byte(addr);
            let al = (self.ax & 0xFF) as u8;

            // Compare AL with memory value (AL - value)
            let result = al.wrapping_sub(value);
            self.set_flags_sub_8(al, value, result);

            // Update DI based on direction flag
            if self.get_flag(FLAG_DIRECTION) {
                self.di = self.di.wrapping_sub(1);
            } else {
                self.di = self.di.wrapping_add(1);
            }
        }
    }

    /// LODS - Load String (opcodes AC-AD)
    /// AC: LODSB - Load byte from DS:SI into AL
    /// AD: LODSW - Load word from DS:SI into AX
    ///
    /// Loads data from DS:SI into AL/AX, then increments/decrements SI based on DF.
    pub(in crate::cpu) fn lods(&mut self, opcode: u8, memory: &Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            // LODSW - Load word
            let addr = Self::physical_address(self.ds, self.si);
            self.ax = memory.read_word(addr);

            // Update SI based on direction flag
            if self.get_flag(FLAG_DIRECTION) {
                self.si = self.si.wrapping_sub(2);
            } else {
                self.si = self.si.wrapping_add(2);
            }
        } else {
            // LODSB - Load byte
            let addr = Self::physical_address(self.ds, self.si);
            let value = memory.read_byte(addr);
            self.ax = (self.ax & 0xFF00) | (value as u16);

            // Update SI based on direction flag
            if self.get_flag(FLAG_DIRECTION) {
                self.si = self.si.wrapping_sub(1);
            } else {
                self.si = self.si.wrapping_add(1);
            }
        }
    }

    /// STOS - Store String (opcodes AA-AB)
    /// AA: STOSB - Store AL into byte at ES:DI
    /// AB: STOSW - Store AX into word at ES:DI
    ///
    /// Stores AL/AX into ES:DI, then increments/decrements DI based on DF.
    pub(in crate::cpu) fn stos(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;

        if is_word {
            // STOSW - Store word
            let addr = Self::physical_address(self.es, self.di);
            memory.write_word(addr, self.ax);

            // Update DI based on direction flag
            if self.get_flag(FLAG_DIRECTION) {
                self.di = self.di.wrapping_sub(2);
            } else {
                self.di = self.di.wrapping_add(2);
            }
        } else {
            // STOSB - Store byte
            let addr = Self::physical_address(self.es, self.di);
            let al = (self.ax & 0xFF) as u8;
            memory.write_byte(addr, al);

            // Update DI based on direction flag
            if self.get_flag(FLAG_DIRECTION) {
                self.di = self.di.wrapping_sub(1);
            } else {
                self.di = self.di.wrapping_add(1);
            }
        }
    }

    /// CLD - Clear Direction Flag (opcode FC)
    /// Sets DF to 0, causing string operations to increment SI/DI (forward direction)
    pub(in crate::cpu) fn cld(&mut self) {
        self.set_flag(FLAG_DIRECTION, false);
    }

    /// STD - Set Direction Flag (opcode FD)
    /// Sets DF to 1, causing string operations to decrement SI/DI (backward direction)
    pub(in crate::cpu) fn std_flag(&mut self) {
        self.set_flag(FLAG_DIRECTION, true);
    }

    /// Helper function to set flags for 8-bit subtraction
    /// Used by CMPS and SCAS
    fn set_flags_sub_8(&mut self, left: u8, right: u8, result: u8) {
        // Zero, Sign, Parity flags
        self.set_flag(FLAG_ZERO, result == 0);
        self.set_flag(FLAG_SIGN, (result & 0x80) != 0);
        self.set_flag(FLAG_PARITY, result.count_ones() % 2 == 0);

        // Carry flag (set if borrow occurred)
        self.set_flag(FLAG_CARRY, left < right);

        // Auxiliary carry (borrow from bit 3)
        let aux_carry = (left & 0x0F) < (right & 0x0F);
        self.set_flag(FLAG_AUXILIARY, aux_carry);

        // Overflow flag (signed overflow)
        let left_sign = (left & 0x80) != 0;
        let right_sign = (right & 0x80) != 0;
        let result_sign = (result & 0x80) != 0;
        let overflow = left_sign != right_sign && left_sign != result_sign;
        self.set_flag(FLAG_OVERFLOW, overflow);
    }

    /// Helper function to set flags for 16-bit subtraction
    /// Used by CMPS and SCAS
    fn set_flags_sub_16(&mut self, left: u16, right: u16, result: u16) {
        // Zero, Sign, Parity flags (parity on low byte only)
        self.set_flag(FLAG_ZERO, result == 0);
        self.set_flag(FLAG_SIGN, (result & 0x8000) != 0);
        self.set_flag(FLAG_PARITY, (result as u8).count_ones() % 2 == 0);

        // Carry flag (set if borrow occurred)
        self.set_flag(FLAG_CARRY, left < right);

        // Auxiliary carry (borrow from bit 3)
        let aux_carry = (left & 0x0F) < (right & 0x0F);
        self.set_flag(FLAG_AUXILIARY, aux_carry);

        // Overflow flag (signed overflow)
        let left_sign = (left & 0x8000) != 0;
        let right_sign = (right & 0x8000) != 0;
        let result_sign = (result & 0x8000) != 0;
        let overflow = left_sign != right_sign && left_sign != result_sign;
        self.set_flag(FLAG_OVERFLOW, overflow);
    }
}
