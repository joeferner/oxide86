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
        self.fpu_status_word = (self.fpu_status_word & !0x3800) | ((self.fpu_top as u16) << 11);
    }

    /// Pop the FPU stack, incrementing TOP.
    fn fpu_pop(&mut self) {
        self.fpu_top = self.fpu_top.wrapping_add(1) & 7;
        self.fpu_status_word = (self.fpu_status_word & !0x3800) | ((self.fpu_top as u16) << 11);
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
                if sign != 0 {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                }
            } else {
                f64::NAN
            };
        }

        let exp64 = exp80 - 16383 + 1023;
        if exp64 <= 0 {
            return if sign != 0 { -0.0f64 } else { 0.0f64 };
        }
        if exp64 >= 0x7FF {
            return if sign != 0 {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
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
        bus.memory_write_u16(addr + 6, 0); // IP offset (not tracked)
        bus.memory_write_u16(addr + 8, 0); // CS / opcode
        bus.memory_write_u16(addr + 10, 0); // operand offset
        bus.memory_write_u16(addr + 12, 0); // operand CS
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
                    // FLDL2T (D9 E9: reg=5, rm=1): push log2(10)
                    (0xD9, 5, 1) => self.fpu_push(consts::LOG2_10),
                    // FLDLG2 (D9 EC: reg=5, rm=4): push log10(2)
                    (0xD9, 5, 4) => self.fpu_push(consts::LOG10_2),
                    // FCHS (D9 E0: reg=4, rm=0)
                    (0xD9, 4, 0) => {
                        self.fpu_stack[self.fpu_top as usize] =
                            -self.fpu_stack[self.fpu_top as usize];
                    }
                    // FABS (D9 E1: reg=4, rm=1)
                    (0xD9, 4, 1) => {
                        self.fpu_stack[self.fpu_top as usize] =
                            self.fpu_stack[self.fpu_top as usize].abs();
                    }
                    // FSQRT (D9 FA: reg=7, rm=2)
                    (0xD9, 7, 2) => {
                        self.fpu_stack[self.fpu_top as usize] =
                            self.fpu_stack[self.fpu_top as usize].sqrt();
                    }
                    // FRNDINT (D9 FC: reg=7, rm=4) — round using RC from control word
                    (0xD9, 7, 4) => {
                        let rc = (self.fpu_control_word >> 10) & 0x3;
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        self.fpu_stack[self.fpu_top as usize] = match rc {
                            0 => st0.round(), // round to nearest
                            1 => st0.floor(), // round down
                            2 => st0.ceil(),  // round up
                            _ => st0.trunc(), // truncate toward zero
                        };
                    }
                    // F2XM1 (D9 F0: reg=6, rm=0): ST(0) = 2^ST(0) - 1
                    (0xD9, 6, 0) => {
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        self.fpu_stack[self.fpu_top as usize] = st0.exp2() - 1.0;
                    }
                    // FYL2X (D9 F1: reg=6, rm=1): ST(1) = ST(1) * log2(ST(0)), pop
                    (0xD9, 6, 1) => {
                        let top = self.fpu_top as usize;
                        let st1 = self.fpu_top.wrapping_add(1) as usize & 7;
                        self.fpu_stack[st1] *= self.fpu_stack[top].log2();
                        self.fpu_pop();
                    }
                    // FPTAN (D9 F2: reg=6, rm=2): ST(0) = tan(ST(0)), push 1.0
                    (0xD9, 6, 2) => {
                        self.fpu_stack[self.fpu_top as usize] =
                            self.fpu_stack[self.fpu_top as usize].tan();
                        self.fpu_push(1.0);
                    }
                    // FPATAN (D9 F3: reg=6, rm=3): ST(1) = atan2(ST(1), ST(0)), pop
                    (0xD9, 6, 3) => {
                        let top = self.fpu_top as usize;
                        let st1 = self.fpu_top.wrapping_add(1) as usize & 7;
                        let y = self.fpu_stack[st1];
                        let x = self.fpu_stack[top];
                        self.fpu_stack[st1] = y.atan2(x);
                        self.fpu_pop();
                    }
                    // FADDP ST(i),ST (DE /0): ST(i) = ST(i) + ST(0), pop
                    (0xDE, 0, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] += self.fpu_stack[top];
                        self.fpu_pop();
                    }
                    // FMULP ST(i),ST (DE /1): ST(i) = ST(i) * ST(0), pop
                    (0xDE, 1, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] *= self.fpu_stack[top];
                        self.fpu_pop();
                    }
                    // FSUBRP ST(i),ST (DE /4): ST(i) = ST(0) - ST(i), pop
                    (0xDE, 4, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[top] - self.fpu_stack[dest];
                        self.fpu_pop();
                    }
                    // FSUBP ST(i),ST (DE /5): ST(i) = ST(i) - ST(0), pop
                    (0xDE, 5, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] -= self.fpu_stack[top];
                        self.fpu_pop();
                    }
                    // FDIVRP ST(i),ST (DE /6): ST(i) = ST(0) / ST(i), pop
                    (0xDE, 6, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[top] / self.fpu_stack[dest];
                        self.fpu_pop();
                    }
                    // FDIVP ST(i),ST (DE /7): ST(i) = ST(i) / ST(0), pop
                    (0xDE, 7, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] /= self.fpu_stack[top];
                        self.fpu_pop();
                    }
                    // FCOMP ST(i) (D8 /3 rm=i): compare ST(0) vs ST(i), set CC, pop once
                    (0xD8, 3, i) => {
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        let sti = self.fpu_stack[self.fpu_top.wrapping_add(i) as usize & 7];
                        self.fpu_set_cc(st0, sti);
                        self.fpu_pop();
                    }
                    // FCOMPP (DE D9: reg=3, rm=1): compare ST(0) vs ST(1), set CC, pop twice
                    (0xDE, 3, 1) => {
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        let st1 = self.fpu_stack[self.fpu_top.wrapping_add(1) as usize & 7];
                        self.fpu_set_cc(st0, st1);
                        self.fpu_pop();
                        self.fpu_pop();
                    }
                    // FXAM (D9 E5: reg=4, rm=5): classify ST(0) into C3/C2/C1/C0
                    (0xD9, 4, 5) => {
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        self.fpu_status_word &= !0x4700; // clear C3/C2/C1/C0
                        if st0.is_sign_negative() {
                            self.fpu_status_word |= 0x0200; // C1 = sign bit
                        }
                        if st0.is_nan() {
                            self.fpu_status_word |= 0x0100; // NaN: C0=1
                        } else if st0.is_infinite() {
                            self.fpu_status_word |= 0x0500; // Infinity: C2=1, C0=1
                        } else if st0 == 0.0 {
                            self.fpu_status_word |= 0x4000; // Zero: C3=1
                        } else if st0.is_subnormal() {
                            self.fpu_status_word |= 0x4400; // Denormal: C3=1, C2=1
                        } else {
                            self.fpu_status_word |= 0x0400; // Normal: C2=1
                        }
                    }
                    // FNCLEX (DB E2: reg=4, rm=2): clear exception flags and busy flag
                    (0xDB, 4, 2) => {
                        self.fpu_status_word &= !0x80FF;
                    }
                    // FFREE ST(i) (DD C0+i: reg=0): mark register as empty (tag word not tracked)
                    (0xDD, 0, _) => {}
                    // FDECSTP (D9 F6: reg=6, rm=6): decrement TOP
                    (0xD9, 6, 6) => {
                        self.fpu_top = self.fpu_top.wrapping_sub(1) & 7;
                        self.fpu_status_word =
                            (self.fpu_status_word & !0x3800) | ((self.fpu_top as u16) << 11);
                    }
                    // FINCSTP (D9 F7: reg=6, rm=7): increment TOP
                    (0xD9, 6, 7) => {
                        self.fpu_top = self.fpu_top.wrapping_add(1) & 7;
                        self.fpu_status_word =
                            (self.fpu_status_word & !0x3800) | ((self.fpu_top as u16) << 11);
                    }
                    // FXTRACT (D9 F4: reg=6, rm=4): ST(0)=exponent, push significand
                    (0xD9, 6, 4) => {
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        let exp = st0.abs().log2().floor();
                        let sig = st0 / exp.exp2();
                        self.fpu_stack[self.fpu_top as usize] = exp;
                        self.fpu_push(sig);
                    }
                    // FPREM (D9 F8: reg=7, rm=0): ST(0) = ST(0) - TRUNC(ST(0)/ST(1))*ST(1)
                    (0xD9, 7, 0) => {
                        let top = self.fpu_top as usize;
                        let st1 = self.fpu_top.wrapping_add(1) as usize & 7;
                        let dividend = self.fpu_stack[top];
                        let divisor = self.fpu_stack[st1];
                        let q = (dividend / divisor).trunc();
                        self.fpu_stack[top] = dividend - q * divisor;
                    }
                    // FYL2XP1 (D9 F9: reg=7, rm=1): ST(1) = ST(1)*log2(ST(0)+1), pop
                    (0xD9, 7, 1) => {
                        let top = self.fpu_top as usize;
                        let st1 = self.fpu_top.wrapping_add(1) as usize & 7;
                        self.fpu_stack[st1] *= (self.fpu_stack[top] + 1.0).log2();
                        self.fpu_pop();
                    }
                    // FSCALE (D9 FD: reg=7, rm=5): ST(0) = ST(0) * 2^TRUNC(ST(1))
                    (0xD9, 7, 5) => {
                        let top = self.fpu_top as usize;
                        let st1 = self.fpu_top.wrapping_add(1) as usize & 7;
                        self.fpu_stack[top] *= self.fpu_stack[st1].trunc().exp2();
                    }
                    // FADD ST,ST(i) (D8 /0 rm=i): ST(0) = ST(0) + ST(i), no pop
                    (0xD8, 0, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] += self.fpu_stack[sti];
                    }
                    // FMUL ST,ST(i) (D8 /1 rm=i): ST(0) = ST(0) * ST(i), no pop
                    (0xD8, 1, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] *= self.fpu_stack[sti];
                    }
                    // FSUB ST,ST(i) (D8 /4 rm=i): ST(0) = ST(0) - ST(i), no pop
                    (0xD8, 4, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] -= self.fpu_stack[sti];
                    }
                    // FSUBR ST,ST(i) (D8 /5 rm=i): ST(0) = ST(i) - ST(0), no pop
                    (0xD8, 5, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] = self.fpu_stack[sti] - self.fpu_stack[top];
                    }
                    // FDIV ST,ST(i) (D8 /6 rm=i): ST(0) = ST(0) / ST(i), no pop
                    (0xD8, 6, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] /= self.fpu_stack[sti];
                    }
                    // FDIVR ST,ST(i) (D8 /7 rm=i): ST(0) = ST(i) / ST(0), no pop
                    (0xD8, 7, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] = self.fpu_stack[sti] / self.fpu_stack[top];
                    }
                    // FADD ST(i),ST (DC /0 rm=i): ST(i) = ST(i) + ST(0), no pop
                    (0xDC, 0, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] += self.fpu_stack[top];
                    }
                    // FMUL ST(i),ST (DC /1 rm=i): ST(i) = ST(i) * ST(0), no pop
                    (0xDC, 1, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] *= self.fpu_stack[top];
                    }
                    // FSUBR ST(i),ST (DC /4 rm=i): ST(i) = ST(0) - ST(i), no pop
                    (0xDC, 4, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[top] - self.fpu_stack[dest];
                    }
                    // FSUB ST(i),ST (DC /5 rm=i): ST(i) = ST(i) - ST(0), no pop
                    (0xDC, 5, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] -= self.fpu_stack[top];
                    }
                    // FDIVR ST(i),ST (DC /6 rm=i): ST(i) = ST(0) / ST(i), no pop
                    (0xDC, 6, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[top] / self.fpu_stack[dest];
                    }
                    // FDIV ST(i),ST (DC /7 rm=i): ST(i) = ST(i) / ST(0), no pop
                    (0xDC, 7, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] /= self.fpu_stack[top];
                    }
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
                    // FADD m32 (D8 /0): ST(0) += m32
                    (0xD8, 0) => {
                        self.fpu_stack[self.fpu_top as usize] += Self::fpu_read_m32(bus, addr);
                    }
                    // FMUL m32 (D8 /1): ST(0) *= m32
                    (0xD8, 1) => {
                        self.fpu_stack[self.fpu_top as usize] *= Self::fpu_read_m32(bus, addr);
                    }
                    // FCOMP m32 (D8 /3): compare ST(0) vs m32, set CC, pop
                    (0xD8, 3) => {
                        let other = Self::fpu_read_m32(bus, addr);
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        self.fpu_set_cc(st0, other);
                        self.fpu_pop();
                    }
                    // FSUB m32 (D8 /4): ST(0) -= m32
                    (0xD8, 4) => {
                        self.fpu_stack[self.fpu_top as usize] -= Self::fpu_read_m32(bus, addr);
                    }
                    // FSUBR m32 (D8 /5): ST(0) = m32 - ST(0)
                    (0xD8, 5) => {
                        let m = Self::fpu_read_m32(bus, addr);
                        self.fpu_stack[self.fpu_top as usize] =
                            m - self.fpu_stack[self.fpu_top as usize];
                    }
                    // FDIV m32 (D8 /6): ST(0) /= m32
                    (0xD8, 6) => {
                        self.fpu_stack[self.fpu_top as usize] /= Self::fpu_read_m32(bus, addr);
                    }
                    // FDIVR m32 (D8 /7): ST(0) = m32 / ST(0)
                    (0xD8, 7) => {
                        let m = Self::fpu_read_m32(bus, addr);
                        self.fpu_stack[self.fpu_top as usize] =
                            m / self.fpu_stack[self.fpu_top as usize];
                    }
                    // FADD m64 (DC /0): ST(0) += m64
                    (0xDC, 0) => {
                        self.fpu_stack[self.fpu_top as usize] += Self::fpu_read_m64(bus, addr);
                    }
                    // FMUL m64 (DC /1): ST(0) *= m64
                    (0xDC, 1) => {
                        self.fpu_stack[self.fpu_top as usize] *= Self::fpu_read_m64(bus, addr);
                    }
                    // FCOMP m64 (DC /3): compare ST(0) vs m64, set CC, pop
                    (0xDC, 3) => {
                        let other = Self::fpu_read_m64(bus, addr);
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        self.fpu_set_cc(st0, other);
                        self.fpu_pop();
                    }
                    // FSUBR m64 (DC /4): ST(0) = m64 - ST(0)
                    (0xDC, 4) => {
                        let m = Self::fpu_read_m64(bus, addr);
                        self.fpu_stack[self.fpu_top as usize] =
                            m - self.fpu_stack[self.fpu_top as usize];
                    }
                    // FSUB m64 (DC /5): ST(0) -= m64
                    (0xDC, 5) => {
                        self.fpu_stack[self.fpu_top as usize] -= Self::fpu_read_m64(bus, addr);
                    }
                    // FDIVR m64 (DC /6): ST(0) = m64 / ST(0)
                    (0xDC, 6) => {
                        let m = Self::fpu_read_m64(bus, addr);
                        self.fpu_stack[self.fpu_top as usize] =
                            m / self.fpu_stack[self.fpu_top as usize];
                    }
                    // FDIV m64 (DC /7): ST(0) /= m64
                    (0xDC, 7) => {
                        self.fpu_stack[self.fpu_top as usize] /= Self::fpu_read_m64(bus, addr);
                    }
                    // FLD m80 (DB /5): load 80-bit extended float and push
                    (0xDB, 5) => {
                        let mut bytes = [0u8; 10];
                        for (i, b) in bytes.iter_mut().enumerate() {
                            *b = bus.memory_read_u8(addr + i);
                        }
                        self.fpu_push(Self::f80_to_f64(bytes));
                    }
                    // FSTP m80 (DB /7): store 80-bit extended float and pop
                    (0xDB, 7) => {
                        let bytes = Self::f64_to_f80(self.fpu_stack[self.fpu_top as usize]);
                        for (i, &b) in bytes.iter().enumerate() {
                            bus.memory_write_u8(addr + i, b);
                        }
                        self.fpu_pop();
                    }
                    // FILD m64 (DF /5): load 64-bit signed integer and push as float
                    (0xDF, 5) => {
                        let w0 = bus.memory_read_u16(addr) as u64;
                        let w1 = bus.memory_read_u16(addr + 2) as u64;
                        let w2 = bus.memory_read_u16(addr + 4) as u64;
                        let w3 = bus.memory_read_u16(addr + 6) as u64;
                        let bits = w0 | (w1 << 16) | (w2 << 32) | (w3 << 48);
                        self.fpu_push(bits as i64 as f64);
                    }
                    // FISTP m64 (DF /7): store ST(0) as 64-bit signed integer and pop
                    (0xDF, 7) => {
                        let i = self.fpu_stack[self.fpu_top as usize].round() as i64 as u64;
                        bus.memory_write_u32(addr, i as u32);
                        bus.memory_write_u32(addr + 4, (i >> 32) as u32);
                        self.fpu_pop();
                    }
                    // FLDENV m14 (D9 /4): restore 14-byte FPU environment from memory
                    (0xD9, 4) => {
                        self.fpu_control_word = bus.memory_read_u16(addr);
                        let sw = bus.memory_read_u16(addr + 2);
                        self.fpu_status_word = sw;
                        self.fpu_top = ((sw >> 11) & 7) as u8;
                    }
                    // FNSTENV m14 (D9 /6): save 14-byte FPU environment to memory
                    (0xD9, 6) => {
                        bus.memory_write_u16(addr, self.fpu_control_word);
                        bus.memory_write_u16(addr + 2, self.fpu_status_word);
                        bus.memory_write_u16(addr + 4, 0xFFFF); // tag word: all empty
                        bus.memory_write_u16(addr + 6, 0); // IP offset (not tracked)
                        bus.memory_write_u16(addr + 8, 0); // CS/opcode
                        bus.memory_write_u16(addr + 10, 0); // operand offset
                        bus.memory_write_u16(addr + 12, 0); // operand CS
                    }
                    // FNSTSW m16 (DD /7): store status word to memory
                    (0xDD, 7) => {
                        bus.memory_write_u16(addr, self.fpu_status_word);
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
