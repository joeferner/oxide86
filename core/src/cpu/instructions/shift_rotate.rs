use super::super::{Cpu, cpu_flag};
use crate::memory::Memory;

impl Cpu {
    /// Shift/Rotate Group 2 (opcodes 0xC0, 0xC1, 0xD0-0xD3)
    /// C0: Shift r/m8, imm8 (80186+)
    /// C1: Shift r/m16, imm8 (80186+)
    /// D0: Shift r/m8, 1
    /// D1: Shift r/m16, 1
    /// D2: Shift r/m8, CL
    /// D3: Shift r/m16, CL
    pub(in crate::cpu) fn shift_rotate_group(&mut self, opcode: u8, memory: &mut Memory) {
        let is_word = opcode & 0x01 != 0;
        let modrm = self.fetch_byte(memory);
        let (mode, operation, rm, addr, _seg) = self.decode_modrm(modrm, memory);

        // Determine shift count
        let count = match opcode {
            0xC0 | 0xC1 => self.fetch_byte(memory), // imm8
            0xD0 | 0xD1 => 1,                       // shift by 1
            0xD2 | 0xD3 => self.get_reg8(1) & 0x1F, // CL (masked to 5 bits for 8086)
            _ => unreachable!(),
        };

        if is_word {
            // 16-bit shift/rotate
            let value = self.read_rm16(mode, rm, addr, memory);
            let result = match operation {
                0 => self.rol_16(value, count),
                1 => self.ror_16(value, count),
                2 => self.rcl_16(value, count),
                3 => self.rcr_16(value, count),
                4 | 6 => self.shl_16(value, count), // SAL is same as SHL
                5 => self.shr_16(value, count),
                7 => self.sar_16(value, count),
                _ => unreachable!(),
            };
            self.write_rm16(mode, rm, addr, result, memory);
        } else {
            // 8-bit shift/rotate
            let value = self.read_rm8(mode, rm, addr, memory);
            let result = match operation {
                0 => self.rol_8(value, count),
                1 => self.ror_8(value, count),
                2 => self.rcl_8(value, count),
                3 => self.rcr_8(value, count),
                4 | 6 => self.shl_8(value, count), // SAL is same as SHL
                5 => self.shr_8(value, count),
                7 => self.sar_8(value, count),
                _ => unreachable!(),
            };
            self.write_rm8(mode, rm, addr, result, memory);
        }
    }

    // 8-bit shift/rotate operations

    /// Shift Left 8-bit (SHL/SAL)
    fn shl_8(&mut self, value: u8, count: u8) -> u8 {
        if count == 0 {
            return value;
        }

        let count = count & 0x1F; // Mask to 5 bits
        if count > 8 {
            // All bits shifted out
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flags_8(0);
            self.set_flag(cpu_flag::OVERFLOW, false);
            return 0;
        }

        let result = value.wrapping_shl(count as u32);

        // Carry is the last bit shifted out
        let carry = if count <= 8 {
            (value >> (8 - count)) & 1 != 0
        } else {
            false
        };

        self.set_flag(cpu_flag::CARRY, carry);
        self.set_flags_8(result);

        // Overflow is defined only for single-bit shifts
        if count == 1 {
            // OF = MSB changed (XOR of original and result MSB)
            let overflow = ((value ^ result) & 0x80) != 0;
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }

    /// Shift Right Logical 8-bit (SHR)
    fn shr_8(&mut self, value: u8, count: u8) -> u8 {
        if count == 0 {
            return value;
        }

        let count = count & 0x1F; // Mask to 5 bits
        if count > 8 {
            // All bits shifted out
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flags_8(0);
            self.set_flag(cpu_flag::OVERFLOW, false);
            return 0;
        }

        let result = value.wrapping_shr(count as u32);

        // Carry is the last bit shifted out
        let carry = if count <= 8 {
            (value >> (count - 1)) & 1 != 0
        } else {
            false
        };

        self.set_flag(cpu_flag::CARRY, carry);
        self.set_flags_8(result);

        // Overflow is defined only for single-bit shifts
        if count == 1 {
            // OF = MSB of original value
            let overflow = (value & 0x80) != 0;
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }

    /// Shift Right Arithmetic 8-bit (SAR)
    fn sar_8(&mut self, value: u8, count: u8) -> u8 {
        if count == 0 {
            return value;
        }

        let count = count & 0x1F; // Mask to 5 bits
        let sign_bit = value & 0x80;

        // For counts >= 8, result is all sign bits
        if count >= 8 {
            let result = if sign_bit != 0 { 0xFF } else { 0 };
            self.set_flag(cpu_flag::CARRY, sign_bit != 0);
            self.set_flags_8(result);
            self.set_flag(cpu_flag::OVERFLOW, false);
            return result;
        }

        let result = ((value as i8) >> count) as u8;

        // Carry is the last bit shifted out
        let carry = (value >> (count - 1)) & 1 != 0;

        self.set_flag(cpu_flag::CARRY, carry);
        self.set_flags_8(result);

        // Overflow is always 0 for SAR
        if count == 1 {
            self.set_flag(cpu_flag::OVERFLOW, false);
        }

        result
    }

    /// Rotate Left 8-bit (ROL)
    fn rol_8(&mut self, value: u8, count: u8) -> u8 {
        if count == 0 {
            return value;
        }

        let count = (count & 0x1F) % 8; // Mask to 5 bits, then mod 8
        if count == 0 {
            return value;
        }

        let result = value.rotate_left(count as u32);

        // Carry is the LSB of result (bit that was rotated from MSB)
        let carry = (result & 1) != 0;
        self.set_flag(cpu_flag::CARRY, carry);

        // Overflow is defined only for single-bit rotates
        if count == 1 {
            // OF = MSB XOR CF
            let overflow = ((result >> 7) & 1) != (result & 1);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }

    /// Rotate Right 8-bit (ROR)
    fn ror_8(&mut self, value: u8, count: u8) -> u8 {
        if count == 0 {
            return value;
        }

        let count = (count & 0x1F) % 8; // Mask to 5 bits, then mod 8
        if count == 0 {
            return value;
        }

        let result = value.rotate_right(count as u32);

        // Carry is the MSB of result (bit that was rotated from LSB)
        let carry = (result & 0x80) != 0;
        self.set_flag(cpu_flag::CARRY, carry);

        // Overflow is defined only for single-bit rotates
        if count == 1 {
            // OF = MSB XOR (MSB-1)
            let overflow = ((result >> 7) ^ (result >> 6)) & 1 != 0;
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }

    /// Rotate through Carry Left 8-bit (RCL)
    fn rcl_8(&mut self, value: u8, count: u8) -> u8 {
        if count == 0 {
            return value;
        }

        let count = (count & 0x1F) % 9; // Mask to 5 bits, then mod 9 (8 bits + carry)
        if count == 0 {
            return value;
        }

        let mut result = value;
        let mut carry = self.get_flag(cpu_flag::CARRY);

        for _ in 0..count {
            let new_carry = (result & 0x80) != 0;
            result = (result << 1) | (if carry { 1 } else { 0 });
            carry = new_carry;
        }

        self.set_flag(cpu_flag::CARRY, carry);

        // Overflow is defined only for single-bit rotates
        if count == 1 {
            // OF = MSB XOR CF
            let overflow = ((result >> 7) & 1) != (if carry { 1 } else { 0 });
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }

    /// Rotate through Carry Right 8-bit (RCR)
    fn rcr_8(&mut self, value: u8, count: u8) -> u8 {
        if count == 0 {
            return value;
        }

        let count = (count & 0x1F) % 9; // Mask to 5 bits, then mod 9 (8 bits + carry)
        if count == 0 {
            return value;
        }

        let mut result = value;
        let mut carry = self.get_flag(cpu_flag::CARRY);

        for _ in 0..count {
            let new_carry = (result & 1) != 0;
            result = (result >> 1) | (if carry { 0x80 } else { 0 });
            carry = new_carry;
        }

        self.set_flag(cpu_flag::CARRY, carry);

        // Overflow is defined only for single-bit rotates
        if count == 1 {
            // OF = MSB XOR (MSB-1)
            let overflow = ((result >> 7) ^ (result >> 6)) & 1 != 0;
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }

    // 16-bit shift/rotate operations

    /// Shift Left 16-bit (SHL/SAL)
    fn shl_16(&mut self, value: u16, count: u8) -> u16 {
        if count == 0 {
            return value;
        }

        let count = count & 0x1F; // Mask to 5 bits
        if count > 16 {
            // All bits shifted out
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flags_16(0);
            self.set_flag(cpu_flag::OVERFLOW, false);
            return 0;
        }

        let result = value.wrapping_shl(count as u32);

        // Carry is the last bit shifted out
        let carry = if count <= 16 {
            (value >> (16 - count)) & 1 != 0
        } else {
            false
        };

        self.set_flag(cpu_flag::CARRY, carry);
        self.set_flags_16(result);

        // Overflow is defined only for single-bit shifts
        if count == 1 {
            // OF = MSB changed (XOR of original and result MSB)
            let overflow = ((value ^ result) & 0x8000) != 0;
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }

    /// Shift Right Logical 16-bit (SHR)
    fn shr_16(&mut self, value: u16, count: u8) -> u16 {
        if count == 0 {
            return value;
        }

        let count = count & 0x1F; // Mask to 5 bits
        if count > 16 {
            // All bits shifted out
            self.set_flag(cpu_flag::CARRY, false);
            self.set_flags_16(0);
            self.set_flag(cpu_flag::OVERFLOW, false);
            return 0;
        }

        let result = value.wrapping_shr(count as u32);

        // Carry is the last bit shifted out
        let carry = if count <= 16 {
            (value >> (count - 1)) & 1 != 0
        } else {
            false
        };

        self.set_flag(cpu_flag::CARRY, carry);
        self.set_flags_16(result);

        // Overflow is defined only for single-bit shifts
        if count == 1 {
            // OF = MSB of original value
            let overflow = (value & 0x8000) != 0;
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }

    /// Shift Right Arithmetic 16-bit (SAR)
    fn sar_16(&mut self, value: u16, count: u8) -> u16 {
        if count == 0 {
            return value;
        }

        let count = count & 0x1F; // Mask to 5 bits
        let sign_bit = value & 0x8000;

        // For counts >= 16, result is all sign bits
        if count >= 16 {
            let result = if sign_bit != 0 { 0xFFFF } else { 0 };
            self.set_flag(cpu_flag::CARRY, sign_bit != 0);
            self.set_flags_16(result);
            self.set_flag(cpu_flag::OVERFLOW, false);
            return result;
        }

        let result = ((value as i16) >> count) as u16;

        // Carry is the last bit shifted out
        let carry = (value >> (count - 1)) & 1 != 0;

        self.set_flag(cpu_flag::CARRY, carry);
        self.set_flags_16(result);

        // Overflow is always 0 for SAR
        if count == 1 {
            self.set_flag(cpu_flag::OVERFLOW, false);
        }

        result
    }

    /// Rotate Left 16-bit (ROL)
    fn rol_16(&mut self, value: u16, count: u8) -> u16 {
        if count == 0 {
            return value;
        }

        let count = (count & 0x1F) % 16; // Mask to 5 bits, then mod 16
        if count == 0 {
            return value;
        }

        let result = value.rotate_left(count as u32);

        // Carry is the LSB of result (bit that was rotated from MSB)
        let carry = (result & 1) != 0;
        self.set_flag(cpu_flag::CARRY, carry);

        // Overflow is defined only for single-bit rotates
        if count == 1 {
            // OF = MSB XOR CF
            let overflow = ((result >> 15) & 1) != (result & 1);
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }

    /// Rotate Right 16-bit (ROR)
    fn ror_16(&mut self, value: u16, count: u8) -> u16 {
        if count == 0 {
            return value;
        }

        let count = (count & 0x1F) % 16; // Mask to 5 bits, then mod 16
        if count == 0 {
            return value;
        }

        let result = value.rotate_right(count as u32);

        // Carry is the MSB of result (bit that was rotated from LSB)
        let carry = (result & 0x8000) != 0;
        self.set_flag(cpu_flag::CARRY, carry);

        // Overflow is defined only for single-bit rotates
        if count == 1 {
            // OF = MSB XOR (MSB-1)
            let overflow = ((result >> 15) ^ (result >> 14)) & 1 != 0;
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }

    /// Rotate through Carry Left 16-bit (RCL)
    fn rcl_16(&mut self, value: u16, count: u8) -> u16 {
        if count == 0 {
            return value;
        }

        let count = (count & 0x1F) % 17; // Mask to 5 bits, then mod 17 (16 bits + carry)
        if count == 0 {
            return value;
        }

        let mut result = value;
        let mut carry = self.get_flag(cpu_flag::CARRY);

        for _ in 0..count {
            let new_carry = (result & 0x8000) != 0;
            result = (result << 1) | (if carry { 1 } else { 0 });
            carry = new_carry;
        }

        self.set_flag(cpu_flag::CARRY, carry);

        // Overflow is defined only for single-bit rotates
        if count == 1 {
            // OF = MSB XOR CF
            let overflow = ((result >> 15) & 1) != (if carry { 1 } else { 0 });
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }

    /// Rotate through Carry Right 16-bit (RCR)
    fn rcr_16(&mut self, value: u16, count: u8) -> u16 {
        if count == 0 {
            return value;
        }

        let count = (count & 0x1F) % 17; // Mask to 5 bits, then mod 17 (16 bits + carry)
        if count == 0 {
            return value;
        }

        let mut result = value;
        let mut carry = self.get_flag(cpu_flag::CARRY);

        for _ in 0..count {
            let new_carry = (result & 1) != 0;
            result = (result >> 1) | (if carry { 0x8000 } else { 0 });
            carry = new_carry;
        }

        self.set_flag(cpu_flag::CARRY, carry);

        // Overflow is defined only for single-bit rotates
        if count == 1 {
            // OF = MSB XOR (MSB-1)
            let overflow = ((result >> 15) ^ (result >> 14)) & 1 != 0;
            self.set_flag(cpu_flag::OVERFLOW, overflow);
        }

        result
    }
}
