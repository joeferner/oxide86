use std::f64::consts;

use crate::{
    bus::Bus,
    cpu::{Cpu, timing},
};

/// Default 8087 control word after FNINIT/FINIT:
///   Bits 11-10 (PC)  = 11 → extended precision (80-bit)
///   Bits  9- 8 (RC)  = 00 → round to nearest (even)
///   Bits  7- 0       = 7F → all six exception masks set (IM PM UM OM ZM DM)
pub(in crate::cpu) const FPU_DEFAULT_CONTROL_WORD: u16 = 0x037F;

impl Cpu {
    /// Set FPU condition codes C0/C2/C3 by comparing `a` against `b`.
    ///
    /// Mapping into the status word (bits 14/10/8):
    ///   a > b  → C3=0 C2=0 C0=0
    ///   a < b  → C3=0 C2=0 C0=1
    ///   a == b → C3=1 C2=0 C0=0
    ///   NaN    → C3=1 C2=1 C0=1 (unordered)
    fn fpu_set_cc(&mut self, a: f64, b: f64) {
        // Clear C0 (bit 8), C2 (bit 10), C3 (bit 14)
        self.fpu_status_word &= !0x4500;
        if a.is_nan() || b.is_nan() {
            self.fpu_status_word |= 0x4500; // unordered: C3=C2=C0=1
        } else if a < b {
            self.fpu_status_word |= 0x0100; // C0=1
        } else if a == b {
            self.fpu_status_word |= 0x4000; // C3=1
        }
        // a > b: all cleared already
    }

    /// Push `value` onto the FPU stack, decrementing TOP.
    fn fpu_push(&mut self, value: f64) {
        self.fpu_top = self.fpu_top.wrapping_sub(1) & 7;
        self.fpu_stack[self.fpu_top as usize] = value;
        self.fpu_status_word =
            (self.fpu_status_word & !0x3800) | ((self.fpu_top as u16) << 11);
    }

    /// Pop the FPU stack, incrementing TOP.
    fn fpu_pop(&mut self) {
        self.fpu_top = self.fpu_top.wrapping_add(1) & 7;
        self.fpu_status_word =
            (self.fpu_status_word & !0x3800) | ((self.fpu_top as u16) << 11);
    }

    /// Reset the FPU to its power-on state (used by FNINIT and FNSAVE).
    fn fpu_reset(&mut self) {
        self.fpu_control_word = FPU_DEFAULT_CONTROL_WORD;
        self.fpu_status_word = 0x0000;
        self.fpu_top = 0;
        self.fpu_stack = [0.0_f64; 8];
    }

    /// Read a 32-bit float from memory at `addr`.
    fn fpu_read_m32(bus: &Bus, addr: usize) -> f64 {
        let bits =
            (bus.memory_read_u16(addr) as u32) | ((bus.memory_read_u16(addr + 2) as u32) << 16);
        f32::from_bits(bits) as f64
    }

    /// Write ST(0) to memory at `addr` as a 32-bit float.
    fn fpu_write_m32(bus: &mut Bus, addr: usize, value: f64) {
        bus.memory_write_u32(addr, (value as f32).to_bits());
    }

    /// Read a 64-bit float from memory at `addr`.
    fn fpu_read_m64(bus: &Bus, addr: usize) -> f64 {
        let w0 = bus.memory_read_u16(addr) as u64;
        let w1 = bus.memory_read_u16(addr + 2) as u64;
        let w2 = bus.memory_read_u16(addr + 4) as u64;
        let w3 = bus.memory_read_u16(addr + 6) as u64;
        f64::from_bits(w0 | (w1 << 16) | (w2 << 32) | (w3 << 48))
    }

    /// Write a 64-bit float to memory at `addr`.
    fn fpu_write_m64(bus: &mut Bus, addr: usize, value: f64) {
        let bits = value.to_bits();
        bus.memory_write_u32(addr, bits as u32);
        bus.memory_write_u32(addr + 4, (bits >> 32) as u32);
    }

    /// Read a 16-bit signed integer from memory at `addr`, convert to f64.
    fn fpu_read_m16int(bus: &Bus, addr: usize) -> f64 {
        bus.memory_read_u16(addr) as i16 as f64
    }

    /// Write ST(0) to memory at `addr` as a 16-bit signed integer (rounded).
    fn fpu_write_m16int(bus: &mut Bus, addr: usize, value: f64) {
        bus.memory_write_u16(addr, value.round() as i16 as u16);
    }

    /// Read a 32-bit signed integer from memory at `addr`, convert to f64.
    fn fpu_read_m32int(bus: &Bus, addr: usize) -> f64 {
        let lo = bus.memory_read_u16(addr) as u32;
        let hi = bus.memory_read_u16(addr + 2) as u32;
        (lo | (hi << 16)) as i32 as f64
    }

    /// Write ST(0) to memory at `addr` as a 32-bit signed integer (rounded).
    fn fpu_write_m32int(bus: &mut Bus, addr: usize, value: f64) {
        bus.memory_write_u32(addr, value.round() as i32 as u32);
    }

    /// Read a 10-byte packed BCD from memory at `addr`, convert to f64.
    /// Format: byte 9 = sign (bit 7), bytes 8-0 = 9 pairs of BCD digits.
    fn fpu_read_bcd(bus: &Bus, addr: usize) -> f64 {
        let mut value: f64 = 0.0;
        let mut multiplier: f64 = 1.0;
        for i in 0..9usize {
            let byte = bus.memory_read_u8(addr + i);
            value += ((byte & 0x0F) as f64) * multiplier;
            multiplier *= 10.0;
            value += ((byte >> 4) as f64) * multiplier;
            multiplier *= 10.0;
        }
        if bus.memory_read_u8(addr + 9) & 0x80 != 0 {
            value = -value;
        }
        value
    }

    /// Write ST(0) to memory at `addr` as a 10-byte packed BCD and pop.
    /// Format: byte 9 = sign (0x00 positive, 0x80 negative), bytes 8-0 = packed digits.
    fn fpu_write_bcd(bus: &mut Bus, addr: usize, value: f64) {
        let sign: u8 = if value < 0.0 { 0x80 } else { 0x00 };
        let mut n = value.abs().round() as u64;
        for i in 0..9usize {
            let lo = (n % 10) as u8;
            n /= 10;
            let hi = (n % 10) as u8;
            n /= 10;
            bus.memory_write_u8(addr + i, lo | (hi << 4));
        }
        bus.memory_write_u8(addr + 9, sign);
    }

    /// Convert an f64 value to the 8087 80-bit extended-precision format (10 bytes, little-endian).
    /// Layout: bytes 0-7 = 64-bit mantissa with explicit integer bit, bytes 8-9 = sign + 15-bit exponent.
    fn f64_to_f80(value: f64) -> [u8; 10] {
        let bits = value.to_bits();
        let sign = (bits >> 63) as u16;
        let exp64 = ((bits >> 52) & 0x7FF) as i32;
        let mantissa64 = bits & 0x000F_FFFF_FFFF_FFFF;

        let (exp80, mantissa80): (u16, u64) = if exp64 == 0 && mantissa64 == 0 {
            (0, 0) // ±zero
        } else if exp64 == 0 {
            // Denormal f64: exponent = 0, no implicit integer bit
            (0, mantissa64 << 11)
        } else if exp64 == 0x7FF {
            // Infinity or NaN
            let m = if mantissa64 == 0 {
                0x8000_0000_0000_0000u64 // infinity: integer bit set, fraction zero
            } else {
                0xC000_0000_0000_0000u64 | (mantissa64 << 11) // NaN: integer + quiet bits set
            };
            (0x7FFF, m)
        } else {
            // Normal: bias 1023 → bias 16383, shift mantissa left 11 bits and add explicit integer bit
            let e = (exp64 - 1023 + 16383) as u16;
            let m = 0x8000_0000_0000_0000u64 | (mantissa64 << 11);
            (e, m)
        };

        let mut result = [0u8; 10];
        result[0..8].copy_from_slice(&mantissa80.to_le_bytes());
        result[8..10].copy_from_slice(&(exp80 | (sign << 15)).to_le_bytes());
        result
    }

    /// Convert an 8087 80-bit extended-precision value (10 bytes, little-endian) to f64.
    fn f80_to_f64(bytes: [u8; 10]) -> f64 {
        let exp_sign = u16::from_le_bytes([bytes[8], bytes[9]]);
        let mantissa80 = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let sign = (exp_sign >> 15) as u64;
        let exp80 = (exp_sign & 0x7FFF) as i32;

        if exp80 == 0 {
            return if sign != 0 { -0.0f64 } else { 0.0f64 };
        }
        if exp80 == 0x7FFF {
            return if mantissa80 & 0x7FFF_FFFF_FFFF_FFFF == 0 {
                if sign != 0 { f64::NEG_INFINITY } else { f64::INFINITY }
            } else {
                f64::NAN
            };
        }

        let exp64 = exp80 - 16383 + 1023;
        if exp64 <= 0 {
            return if sign != 0 { -0.0f64 } else { 0.0f64 };
        }
        if exp64 >= 0x7FF {
            return if sign != 0 { f64::NEG_INFINITY } else { f64::INFINITY };
        }

        let mantissa64 = (mantissa80 >> 11) & 0x000F_FFFF_FFFF_FFFF;
        f64::from_bits((sign << 63) | ((exp64 as u64) << 52) | mantissa64)
    }

    /// Write the 94-byte FNSAVE state block to memory at `addr`.
    /// Format (real mode, 8087):
    ///   +0  control word  (2 bytes)
    ///   +2  status word   (2 bytes)
    ///   +4  tag word      (2 bytes)
    ///   +6  IP offset     (2 bytes)
    ///   +8  CS / opcode   (2 bytes)
    ///   +10 operand offset(2 bytes)
    ///   +12 operand CS    (2 bytes)
    ///   +14 R0..R7        (8 × 10 bytes = 80 bytes)
    fn fpu_save_state(&self, bus: &mut Bus, addr: usize) {
        // Header
        bus.memory_write_u16(addr, self.fpu_control_word);
        bus.memory_write_u16(addr + 2, self.fpu_status_word);
        bus.memory_write_u16(addr + 4, 0xFFFF); // tag word: all empty
        bus.memory_write_u16(addr + 6, 0);      // IP offset (not tracked)
        bus.memory_write_u16(addr + 8, 0);      // CS / opcode
        bus.memory_write_u16(addr + 10, 0);     // operand offset
        bus.memory_write_u16(addr + 12, 0);     // operand CS
        // Physical registers R0-R7
        for i in 0..8usize {
            let f80 = Self::f64_to_f80(self.fpu_stack[i]);
            let reg_addr = addr + 14 + i * 10;
            for (j, byte) in f80.iter().enumerate() {
                bus.memory_write_u8(reg_addr + j, *byte);
            }
        }
    }

    /// Read a 94-byte FNSAVE state block from memory at `addr` and restore FPU state.
    fn fpu_load_state(&mut self, bus: &Bus, addr: usize) {
        self.fpu_control_word = bus.memory_read_u16(addr);
        let sw = bus.memory_read_u16(addr + 2);
        self.fpu_status_word = sw;
        self.fpu_top = ((sw >> 11) & 7) as u8;
        // Physical registers R0-R7
        for i in 0..8usize {
            let reg_addr = addr + 14 + i * 10;
            let mut f80 = [0u8; 10];
            for (j, byte) in f80.iter_mut().enumerate() {
                *byte = bus.memory_read_u8(reg_addr + j);
            }
            self.fpu_stack[i] = Self::f80_to_f64(f80);
        }
    }

    /// ESC - Escape to coprocessor (opcodes D8-DF)
    /// Passes instruction to 8087 FPU. Without a coprocessor, this is a NOP
    /// that reads the ModR/M byte and any displacement to maintain bus timing.
    /// With a coprocessor, dispatches the subset of 8087 instructions implemented.
    pub(in crate::cpu) fn esc(&mut self, opcode: u8, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if self.math_coprocessor {
            if mode == 0b11 {
                match (opcode, reg, rm) {
                    // FNINIT (DB /4 rm=3 → DB E3)
                    (0xDB, 4, 3) => self.fpu_reset(),
                    // FNSTSW AX (DF /4 rm=0 → DF E0)
                    (0xDF, 4, 0) => self.ax = self.fpu_status_word,
                    // FLD ST(i) (D9 /0 rm=i → D9 C0+i)
                    (0xD9, 0, i) => {
                        let val = self.fpu_stack[self.fpu_top.wrapping_add(i) as usize & 7];
                        self.fpu_push(val);
                    }
                    // FTST (D9 E4: mode=11, reg=4, rm=4)
                    (0xD9, 4, 4) => {
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        self.fpu_set_cc(st0, 0.0);
                    }
                    // FCOM ST(i) (D8 /2 rm=i → D8 D0+i)
                    (0xD8, 2, i) => {
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        let sti = self.fpu_stack[self.fpu_top.wrapping_add(i) as usize & 7];
                        self.fpu_set_cc(st0, sti);
                    }
                    // FXCH ST(i) (D9 /1 rm=i → D9 C8+i)
                    (0xD9, 1, i) => {
                        let top = self.fpu_top as usize;
                        let other = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack.swap(top, other);
                    }
                    // FLD1 (D9 E8: reg=5, rm=0)
                    (0xD9, 5, 0) => self.fpu_push(1.0),
                    // FLDL2E (D9 EA: reg=5, rm=2)
                    (0xD9, 5, 2) => self.fpu_push(consts::LOG2_E),
                    // FLDPI (D9 EB: reg=5, rm=3)
                    (0xD9, 5, 3) => self.fpu_push(consts::PI),
                    // FLDLN2 (D9 ED: reg=5, rm=5)
                    (0xD9, 5, 5) => self.fpu_push(consts::LN_2),
                    // FLDZ (D9 EE: reg=5, rm=6)
                    (0xD9, 5, 6) => self.fpu_push(0.0),
                    _ => log::warn!(
                        "unimplemented FPU register instruction: opcode={:#04X} reg={} rm={}",
                        opcode,
                        reg,
                        rm
                    ),
                }
            } else {
                match (opcode, reg) {
                    // FCOM m32 (D8 /2)
                    (0xD8, 2) => {
                        let other = Self::fpu_read_m32(bus, addr);
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        self.fpu_set_cc(st0, other);
                    }
                    // FCOM m64 (DC /2)
                    (0xDC, 2) => {
                        let other = Self::fpu_read_m64(bus, addr);
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        self.fpu_set_cc(st0, other);
                    }
                    // FLD m32 (D9 /0)
                    (0xD9, 0) => {
                        let val = Self::fpu_read_m32(bus, addr);
                        self.fpu_push(val);
                    }
                    // FLD m64 (DD /0)
                    (0xDD, 0) => {
                        let val = Self::fpu_read_m64(bus, addr);
                        self.fpu_push(val);
                    }
                    // FST m32 (D9 /2)
                    (0xD9, 2) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_m32(bus, addr, val);
                    }
                    // FST m64 (DD /2)
                    (0xDD, 2) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_m64(bus, addr, val);
                    }
                    // FSTP m32 (D9 /3)
                    (0xD9, 3) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_m32(bus, addr, val);
                        self.fpu_pop();
                    }
                    // FSTP m64 (DD /3)
                    (0xDD, 3) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_m64(bus, addr, val);
                        self.fpu_pop();
                    }
                    // FILD m16 (DF /0)
                    (0xDF, 0) => {
                        let val = Self::fpu_read_m16int(bus, addr);
                        self.fpu_push(val);
                    }
                    // FILD m32 (DB /0)
                    (0xDB, 0) => {
                        let val = Self::fpu_read_m32int(bus, addr);
                        self.fpu_push(val);
                    }
                    // FIST m16 (DF /2)
                    (0xDF, 2) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_m16int(bus, addr, val);
                    }
                    // FIST m32 (DB /2)
                    (0xDB, 2) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_m32int(bus, addr, val);
                    }
                    // FISTP m16 (DF /3)
                    (0xDF, 3) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_m16int(bus, addr, val);
                        self.fpu_pop();
                    }
                    // FISTP m32 (DB /3)
                    (0xDB, 3) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_m32int(bus, addr, val);
                        self.fpu_pop();
                    }
                    // FBLD m80bcd (DF /4)
                    (0xDF, 4) => {
                        let val = Self::fpu_read_bcd(bus, addr);
                        self.fpu_push(val);
                    }
                    // FBSTP m80bcd (DF /6)
                    (0xDF, 6) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_bcd(bus, addr, val);
                        self.fpu_pop();
                    }
                    // FLDCW m16 (D9 /5)
                    (0xD9, 5) => {
                        self.fpu_control_word = bus.memory_read_u16(addr);
                    }
                    // FNSTCW m16 (D9 /7)
                    (0xD9, 7) => {
                        bus.memory_write_u16(addr, self.fpu_control_word);
                    }
                    // FNSAVE m94 (DD /6): save state to memory then reset FPU
                    (0xDD, 6) => {
                        self.fpu_save_state(bus, addr);
                        self.fpu_reset();
                    }
                    // FRSTOR m94 (DD /4): restore state from memory
                    (0xDD, 4) => {
                        self.fpu_load_state(bus, addr);
                    }
                    _ => log::warn!(
                        "unimplemented FPU memory instruction: opcode={:#04X} reg={}",
                        opcode,
                        reg
                    ),
                }
            }
        }

        bus.increment_cycle_count(timing::cycles::ESC)
    }
}
