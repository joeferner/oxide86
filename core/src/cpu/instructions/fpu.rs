use crate::{
    bus::Bus,
    cpu::{Cpu, f80::F80, f80_trig, timing},
};

/// Default 8087 control word after FNINIT/FINIT:
///   Bits 11-10 (PC)  = 11 → extended precision (80-bit)
///   Bits  9- 8 (RC)  = 00 → round to nearest (even)
///   Bits  7- 0       = 7F → all six exception masks set (IM PM UM OM ZM DM)
pub(in crate::cpu) const FPU_DEFAULT_CONTROL_WORD: u16 = 0x037F;

impl Cpu {
    /// Return the current rounding mode from the control word (RC field, bits 11-10).
    /// 0=nearest-even, 1=floor, 2=ceil, 3=truncate
    fn fpu_rc(&self) -> u8 {
        ((self.fpu_control_word >> 10) & 0x3) as u8
    }

    /// Set FPU condition codes C0/C2/C3 by comparing `a` against `b`.
    ///
    /// Mapping into the status word (bits 14/10/8):
    ///   a > b  → C3=0 C2=0 C0=0
    ///   a < b  → C3=0 C2=0 C0=1
    ///   a == b → C3=1 C2=0 C0=0
    ///   NaN    → C3=1 C2=1 C0=1 (unordered)
    fn fpu_set_cc(&mut self, a: F80, b: F80) {
        let (c0, c2, c3) = a.compare_cc(b);
        self.fpu_status_word &= !0x4500;
        if c0 {
            self.fpu_status_word |= 0x0100;
        }
        if c2 {
            self.fpu_status_word |= 0x0400;
        }
        if c3 {
            self.fpu_status_word |= 0x4000;
        }
    }

    /// Push `value` onto the FPU stack, decrementing TOP.
    fn fpu_push(&mut self, value: F80) {
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
        self.fpu_stack = [F80::ZERO; 8];
    }

    /// Read a 32-bit float from memory at `addr`.
    fn fpu_read_m32(bus: &Bus, addr: usize) -> F80 {
        let bits =
            (bus.memory_read_u16(addr) as u32) | ((bus.memory_read_u16(addr + 2) as u32) << 16);
        F80::from_f64(f32::from_bits(bits) as f64)
    }

    /// Write ST(0) to memory at `addr` as a 32-bit float.
    fn fpu_write_m32(bus: &mut Bus, addr: usize, value: F80) {
        bus.memory_write_u32(addr, (value.to_f64() as f32).to_bits());
    }

    /// Read a 64-bit float from memory at `addr`.
    fn fpu_read_m64(bus: &Bus, addr: usize) -> F80 {
        let w0 = bus.memory_read_u16(addr) as u64;
        let w1 = bus.memory_read_u16(addr + 2) as u64;
        let w2 = bus.memory_read_u16(addr + 4) as u64;
        let w3 = bus.memory_read_u16(addr + 6) as u64;
        F80::from_f64(f64::from_bits(w0 | (w1 << 16) | (w2 << 32) | (w3 << 48)))
    }

    /// Write a 64-bit float to memory at `addr`.
    fn fpu_write_m64(bus: &mut Bus, addr: usize, value: F80) {
        let bits = value.to_f64().to_bits();
        bus.memory_write_u32(addr, bits as u32);
        bus.memory_write_u32(addr + 4, (bits >> 32) as u32);
    }

    /// Read a 16-bit signed integer from memory at `addr`, convert to F80.
    fn fpu_read_m16int(bus: &Bus, addr: usize) -> F80 {
        F80::from_i64(bus.memory_read_u16(addr) as i16 as i64)
    }

    /// Write ST(0) to memory at `addr` as a 16-bit signed integer.
    fn fpu_write_m16int(bus: &mut Bus, addr: usize, value: F80, rc: u8) {
        bus.memory_write_u16(addr, value.to_i64(rc) as i16 as u16);
    }

    /// Read a 32-bit signed integer from memory at `addr`, convert to F80.
    fn fpu_read_m32int(bus: &Bus, addr: usize) -> F80 {
        let lo = bus.memory_read_u16(addr) as u32;
        let hi = bus.memory_read_u16(addr + 2) as u32;
        F80::from_i64((lo | (hi << 16)) as i32 as i64)
    }

    /// Write ST(0) to memory at `addr` as a 32-bit signed integer.
    fn fpu_write_m32int(bus: &mut Bus, addr: usize, value: F80, rc: u8) {
        bus.memory_write_u32(addr, value.to_i64(rc) as i32 as u32);
    }

    /// Read a 10-byte packed BCD from memory at `addr`, convert to F80.
    /// Format: byte 9 = sign (bit 7), bytes 8-0 = 9 pairs of BCD digits.
    fn fpu_read_bcd(bus: &Bus, addr: usize) -> F80 {
        let mut value: i64 = 0;
        let mut multiplier: i64 = 1;
        for i in 0..9usize {
            let byte = bus.memory_read_u8(addr + i);
            value += ((byte & 0x0F) as i64) * multiplier;
            multiplier *= 10;
            value += ((byte >> 4) as i64) * multiplier;
            multiplier *= 10;
        }
        if bus.memory_read_u8(addr + 9) & 0x80 != 0 {
            value = -value;
        }
        F80::from_i64(value)
    }

    /// Write ST(0) to memory at `addr` as a 10-byte packed BCD and pop.
    fn fpu_write_bcd(bus: &mut Bus, addr: usize, value: F80) {
        let sign: u8 = if value.is_negative() { 0x80 } else { 0x00 };
        let mut n = value.abs().to_i64(3).unsigned_abs(); // truncate mode
        for i in 0..9usize {
            let lo = (n % 10) as u8;
            n /= 10;
            let hi = (n % 10) as u8;
            n /= 10;
            bus.memory_write_u8(addr + i, lo | (hi << 4));
        }
        bus.memory_write_u8(addr + 9, sign);
    }

    /// Write the 94-byte FNSAVE state block to memory at `addr`.
    fn fpu_save_state(&self, bus: &mut Bus, addr: usize) {
        bus.memory_write_u16(addr, self.fpu_control_word);
        bus.memory_write_u16(addr + 2, self.fpu_status_word);
        bus.memory_write_u16(addr + 4, 0xFFFF); // tag word: all empty
        bus.memory_write_u16(addr + 6, 0); // IP offset (not tracked)
        bus.memory_write_u16(addr + 8, 0); // CS / opcode
        bus.memory_write_u16(addr + 10, 0); // operand offset
        bus.memory_write_u16(addr + 12, 0); // operand CS
        for i in 0..8usize {
            let bytes = self.fpu_stack[i].to_bytes();
            let reg_addr = addr + 14 + i * 10;
            for (j, byte) in bytes.iter().enumerate() {
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
        for i in 0..8usize {
            let reg_addr = addr + 14 + i * 10;
            let mut bytes = [0u8; 10];
            for (j, b) in bytes.iter_mut().enumerate() {
                *b = bus.memory_read_u8(reg_addr + j);
            }
            self.fpu_stack[i] = F80::from_bytes(bytes);
        }
    }

    /// ESC - Escape to coprocessor (opcodes D8-DF)
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
                        self.fpu_set_cc(st0, F80::ZERO);
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
                    (0xD9, 5, 0) => self.fpu_push(F80::ONE),
                    // FLDL2E (D9 EA: reg=5, rm=2)
                    (0xD9, 5, 2) => self.fpu_push(F80::LOG2_E),
                    // FLDPI (D9 EB: reg=5, rm=3)
                    (0xD9, 5, 3) => self.fpu_push(F80::PI),
                    // FLDLN2 (D9 ED: reg=5, rm=5)
                    (0xD9, 5, 5) => self.fpu_push(F80::LN_2),
                    // FLDZ (D9 EE: reg=5, rm=6)
                    (0xD9, 5, 6) => self.fpu_push(F80::ZERO),
                    // FLDL2T (D9 E9: reg=5, rm=1)
                    (0xD9, 5, 1) => self.fpu_push(F80::LOG2_10),
                    // FLDLG2 (D9 EC: reg=5, rm=4)
                    (0xD9, 5, 4) => self.fpu_push(F80::LOG10_2),
                    // FCHS (D9 E0: reg=4, rm=0)
                    (0xD9, 4, 0) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] = self.fpu_stack[top].neg();
                    }
                    // FABS (D9 E1: reg=4, rm=1)
                    (0xD9, 4, 1) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] = self.fpu_stack[top].abs();
                    }
                    // FSQRT (D9 FA: reg=7, rm=2)
                    (0xD9, 7, 2) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] = self.fpu_stack[top].sqrt();
                    }
                    // FRNDINT (D9 FC: reg=7, rm=4) — round using RC from control word
                    (0xD9, 7, 4) => {
                        let rc = self.fpu_rc();
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] = self.fpu_stack[top].round_to_integer(rc);
                    }
                    // F2XM1 (D9 F0: reg=6, rm=0): ST(0) = 2^ST(0) - 1
                    (0xD9, 6, 0) => {
                        let top = self.fpu_top as usize;
                        let v = self.fpu_stack[top].to_f64().exp2() - 1.0;
                        self.fpu_stack[top] = F80::from_f64(v);
                    }
                    // FYL2X (D9 F1: reg=6, rm=1): ST(1) = ST(1) * log2(ST(0)), pop
                    (0xD9, 6, 1) => {
                        let top = self.fpu_top as usize;
                        let st1 = self.fpu_top.wrapping_add(1) as usize & 7;
                        let v = self.fpu_stack[st1].to_f64() * self.fpu_stack[top].to_f64().log2();
                        self.fpu_stack[st1] = F80::from_f64(v);
                        self.fpu_pop();
                    }
                    // FPTAN (D9 F2: reg=6, rm=2): ST(0) = tan(ST(0)), push 1.0
                    // Uses float128 polynomial (BOCHS algorithm) for 8087-compatible precision.
                    (0xD9, 6, 2) => {
                        let top = self.fpu_top as usize;
                        match f80_trig::ftan(self.fpu_stack[top]) {
                            Some(result) => {
                                self.fpu_stack[top] = result;
                                self.fpu_push(F80::ONE);
                            }
                            None => {
                                // Argument out of range: set C2 flag, leave ST(0) unchanged
                                self.fpu_status_word |= 0x0400; // C2 = 1
                            }
                        }
                    }
                    // FPATAN (D9 F3: reg=6, rm=3): ST(1) = atan2(ST(1), ST(0)), pop
                    // Uses float128 polynomial (BOCHS algorithm) for 8087-compatible precision.
                    (0xD9, 6, 3) => {
                        let top = self.fpu_top as usize;
                        let st1 = self.fpu_top.wrapping_add(1) as usize & 7;
                        let x = self.fpu_stack[top]; // ST(0) = x
                        let y = self.fpu_stack[st1]; // ST(1) = y
                        self.fpu_stack[st1] = f80_trig::fpatan(x, y);
                        self.fpu_pop();
                    }
                    // FADDP ST(i),ST (DE /0): ST(i) = ST(i) + ST(0), pop
                    (0xDE, 0, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[dest].add(self.fpu_stack[top]);
                        self.fpu_pop();
                    }
                    // FMULP ST(i),ST (DE /1): ST(i) = ST(i) * ST(0), pop
                    (0xDE, 1, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[dest].mul(self.fpu_stack[top]);
                        self.fpu_pop();
                    }
                    // FSUBRP ST(i),ST (DE /4): ST(i) = ST(0) - ST(i), pop
                    (0xDE, 4, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[top].sub(self.fpu_stack[dest]);
                        self.fpu_pop();
                    }
                    // FSUBP ST(i),ST (DE /5): ST(i) = ST(i) - ST(0), pop
                    (0xDE, 5, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[dest].sub(self.fpu_stack[top]);
                        self.fpu_pop();
                    }
                    // FDIVRP ST(i),ST (DE /6): ST(i) = ST(0) / ST(i), pop
                    (0xDE, 6, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[top].div(self.fpu_stack[dest]);
                        self.fpu_pop();
                    }
                    // FDIVP ST(i),ST (DE /7): ST(i) = ST(i) / ST(0), pop
                    (0xDE, 7, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[dest].div(self.fpu_stack[top]);
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
                        if st0.is_negative() {
                            self.fpu_status_word |= 0x0200; // C1 = sign bit
                        }
                        if st0.is_nan() {
                            self.fpu_status_word |= 0x0100; // NaN: C0=1
                        } else if st0.is_infinite() {
                            self.fpu_status_word |= 0x0500; // Infinity: C2=1, C0=1
                        } else if st0.is_zero() {
                            self.fpu_status_word |= 0x4000; // Zero: C3=1
                        } else if st0.exp == 0 {
                            self.fpu_status_word |= 0x4400; // Denormal: C3=1, C2=1
                        } else {
                            self.fpu_status_word |= 0x0400; // Normal: C2=1
                        }
                    }
                    // FNCLEX (DB E2: reg=4, rm=2): clear exception flags and busy flag
                    (0xDB, 4, 2) => {
                        self.fpu_status_word &= !0x80FF;
                    }
                    // FSTP ST(i) (DD /3 rm=i): copy ST(0) to ST(i), pop
                    (0xDD, 3, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[top];
                        self.fpu_pop();
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
                    // FXTRACT (D9 F4: reg=6, rm=4): ST(0)=unbiased exponent, push significand
                    (0xD9, 6, 4) => {
                        let top = self.fpu_top as usize;
                        let st0 = self.fpu_stack[top];
                        if st0.is_zero() || st0.is_nan() || st0.is_infinite() {
                            // push NaN / propagate
                            self.fpu_push(st0);
                        } else {
                            let exp_val = F80::from_i64(st0.exp as i64 - 16383);
                            // significand: same value but with exp set to 16383 (so value in [1,2))
                            let sig = F80 {
                                sign: st0.sign,
                                exp: 0x3FFF,
                                mant: st0.mant,
                            };
                            self.fpu_stack[top] = exp_val;
                            self.fpu_push(sig);
                        }
                    }
                    // FPREM (D9 F8: reg=7, rm=0): ST(0) = ST(0) - TRUNC(ST(0)/ST(1))*ST(1)
                    (0xD9, 7, 0) => {
                        let top = self.fpu_top as usize;
                        let st1 = self.fpu_top.wrapping_add(1) as usize & 7;
                        let dividend = self.fpu_stack[top].to_f64();
                        let divisor = self.fpu_stack[st1].to_f64();
                        let q = (dividend / divisor).trunc();
                        self.fpu_stack[top] = F80::from_f64(dividend - q * divisor);
                    }
                    // FYL2XP1 (D9 F9: reg=7, rm=1): ST(1) = ST(1)*log2(ST(0)+1), pop
                    (0xD9, 7, 1) => {
                        let top = self.fpu_top as usize;
                        let st1 = self.fpu_top.wrapping_add(1) as usize & 7;
                        let v = self.fpu_stack[st1].to_f64()
                            * (self.fpu_stack[top].to_f64() + 1.0).log2();
                        self.fpu_stack[st1] = F80::from_f64(v);
                        self.fpu_pop();
                    }
                    // FSCALE (D9 FD: reg=7, rm=5): ST(0) = ST(0) * 2^TRUNC(ST(1))
                    (0xD9, 7, 5) => {
                        let top = self.fpu_top as usize;
                        let st1 = self.fpu_top.wrapping_add(1) as usize & 7;
                        let scale = self.fpu_stack[st1].to_f64().trunc() as i32;
                        let st0 = self.fpu_stack[top];
                        if st0.exp == 0 || st0.is_nan() || st0.is_infinite() {
                            // propagate
                        } else {
                            let new_exp = st0.exp as i32 + scale;
                            if new_exp <= 0 {
                                self.fpu_stack[top] =
                                    if st0.sign { F80::NEG_ZERO } else { F80::ZERO };
                            } else if new_exp >= 0x7FFF {
                                self.fpu_stack[top] =
                                    if st0.sign { F80::NEG_INF } else { F80::POS_INF };
                            } else {
                                self.fpu_stack[top] = F80 {
                                    exp: new_exp as u16,
                                    ..st0
                                };
                            }
                        }
                    }
                    // FADD ST,ST(i) (D8 /0 rm=i): ST(0) = ST(0) + ST(i)
                    (0xD8, 0, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] = self.fpu_stack[top].add(self.fpu_stack[sti]);
                    }
                    // FMUL ST,ST(i) (D8 /1 rm=i): ST(0) = ST(0) * ST(i)
                    (0xD8, 1, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] = self.fpu_stack[top].mul(self.fpu_stack[sti]);
                    }
                    // FSUB ST,ST(i) (D8 /4 rm=i): ST(0) = ST(0) - ST(i)
                    (0xD8, 4, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] = self.fpu_stack[top].sub(self.fpu_stack[sti]);
                    }
                    // FSUBR ST,ST(i) (D8 /5 rm=i): ST(0) = ST(i) - ST(0)
                    (0xD8, 5, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] = self.fpu_stack[sti].sub(self.fpu_stack[top]);
                    }
                    // FDIV ST,ST(i) (D8 /6 rm=i): ST(0) = ST(0) / ST(i)
                    (0xD8, 6, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] = self.fpu_stack[top].div(self.fpu_stack[sti]);
                    }
                    // FDIVR ST,ST(i) (D8 /7 rm=i): ST(0) = ST(i) / ST(0)
                    (0xD8, 7, i) => {
                        let top = self.fpu_top as usize;
                        let sti = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[top] = self.fpu_stack[sti].div(self.fpu_stack[top]);
                    }
                    // FADD ST(i),ST (DC /0 rm=i): ST(i) = ST(i) + ST(0)
                    (0xDC, 0, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[dest].add(self.fpu_stack[top]);
                    }
                    // FMUL ST(i),ST (DC /1 rm=i): ST(i) = ST(i) * ST(0)
                    (0xDC, 1, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[dest].mul(self.fpu_stack[top]);
                    }
                    // FSUBR ST(i),ST (DC /4 rm=i): ST(i) = ST(0) - ST(i)
                    (0xDC, 4, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[top].sub(self.fpu_stack[dest]);
                    }
                    // FSUB ST(i),ST (DC /5 rm=i): ST(i) = ST(i) - ST(0)
                    (0xDC, 5, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[dest].sub(self.fpu_stack[top]);
                    }
                    // FDIVR ST(i),ST (DC /6 rm=i): ST(i) = ST(0) / ST(i)
                    (0xDC, 6, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[top].div(self.fpu_stack[dest]);
                    }
                    // FDIV ST(i),ST (DC /7 rm=i): ST(i) = ST(i) / ST(0)
                    (0xDC, 7, i) => {
                        let top = self.fpu_top as usize;
                        let dest = self.fpu_top.wrapping_add(i) as usize & 7;
                        self.fpu_stack[dest] = self.fpu_stack[dest].div(self.fpu_stack[top]);
                    }
                    _ => log::warn!(
                        "unimplemented FPU register instruction: opcode={:#04X} reg={} rm={}",
                        opcode,
                        reg,
                        rm
                    ),
                }
            } else {
                let rc = self.fpu_rc();
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
                        Self::fpu_write_m16int(bus, addr, val, rc);
                    }
                    // FIST m32 (DB /2)
                    (0xDB, 2) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_m32int(bus, addr, val, rc);
                    }
                    // FISTP m16 (DF /3)
                    (0xDF, 3) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_m16int(bus, addr, val, rc);
                        self.fpu_pop();
                    }
                    // FISTP m32 (DB /3)
                    (0xDB, 3) => {
                        let val = self.fpu_stack[self.fpu_top as usize];
                        Self::fpu_write_m32int(bus, addr, val, rc);
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
                    // FIMUL m16 (DE /1): ST(0) *= m16int
                    (0xDE, 1) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].mul(Self::fpu_read_m16int(bus, addr));
                    }
                    // FIADD m32 (DA /0): ST(0) += m32int
                    (0xDA, 0) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].add(Self::fpu_read_m32int(bus, addr));
                    }
                    // FIMUL m32 (DA /1): ST(0) *= m32int
                    (0xDA, 1) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].mul(Self::fpu_read_m32int(bus, addr));
                    }
                    // FICOM m32 (DA /2): compare ST(0) vs m32int, set CC
                    (0xDA, 2) => {
                        let other = Self::fpu_read_m32int(bus, addr);
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        self.fpu_set_cc(st0, other);
                    }
                    // FICOMP m32 (DA /3): compare ST(0) vs m32int, set CC, pop
                    (0xDA, 3) => {
                        let other = Self::fpu_read_m32int(bus, addr);
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        self.fpu_set_cc(st0, other);
                        self.fpu_pop();
                    }
                    // FISUB m32 (DA /4): ST(0) -= m32int
                    (0xDA, 4) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].sub(Self::fpu_read_m32int(bus, addr));
                    }
                    // FISUBR m32 (DA /5): ST(0) = m32int - ST(0)
                    (0xDA, 5) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            Self::fpu_read_m32int(bus, addr).sub(self.fpu_stack[top]);
                    }
                    // FIDIV m32 (DA /6): ST(0) /= m32int
                    (0xDA, 6) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].div(Self::fpu_read_m32int(bus, addr));
                    }
                    // FIDIVR m32 (DA /7): ST(0) = m32int / ST(0)
                    (0xDA, 7) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            Self::fpu_read_m32int(bus, addr).div(self.fpu_stack[top]);
                    }
                    // FADD m32 (D8 /0): ST(0) += m32
                    (0xD8, 0) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].add(Self::fpu_read_m32(bus, addr));
                    }
                    // FMUL m32 (D8 /1): ST(0) *= m32
                    (0xD8, 1) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].mul(Self::fpu_read_m32(bus, addr));
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
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].sub(Self::fpu_read_m32(bus, addr));
                    }
                    // FSUBR m32 (D8 /5): ST(0) = m32 - ST(0)
                    (0xD8, 5) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            Self::fpu_read_m32(bus, addr).sub(self.fpu_stack[top]);
                    }
                    // FDIV m32 (D8 /6): ST(0) /= m32
                    (0xD8, 6) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].div(Self::fpu_read_m32(bus, addr));
                    }
                    // FDIVR m32 (D8 /7): ST(0) = m32 / ST(0)
                    (0xD8, 7) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            Self::fpu_read_m32(bus, addr).div(self.fpu_stack[top]);
                    }
                    // FADD m64 (DC /0): ST(0) += m64
                    (0xDC, 0) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].add(Self::fpu_read_m64(bus, addr));
                    }
                    // FMUL m64 (DC /1): ST(0) *= m64
                    (0xDC, 1) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].mul(Self::fpu_read_m64(bus, addr));
                    }
                    // FCOMP m64 (DC /3): compare ST(0) vs m64, set CC, pop
                    (0xDC, 3) => {
                        let other = Self::fpu_read_m64(bus, addr);
                        let st0 = self.fpu_stack[self.fpu_top as usize];
                        self.fpu_set_cc(st0, other);
                        self.fpu_pop();
                    }
                    // FSUB m64 (DC /4): ST(0) -= m64
                    (0xDC, 4) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].sub(Self::fpu_read_m64(bus, addr));
                    }
                    // FSUBR m64 (DC /5): ST(0) = m64 - ST(0)
                    (0xDC, 5) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            Self::fpu_read_m64(bus, addr).sub(self.fpu_stack[top]);
                    }
                    // FDIV m64 (DC /6): ST(0) /= m64
                    (0xDC, 6) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            self.fpu_stack[top].div(Self::fpu_read_m64(bus, addr));
                    }
                    // FDIVR m64 (DC /7): ST(0) = m64 / ST(0)
                    (0xDC, 7) => {
                        let top = self.fpu_top as usize;
                        self.fpu_stack[top] =
                            Self::fpu_read_m64(bus, addr).div(self.fpu_stack[top]);
                    }
                    // FLD m80 (DB /5): load 80-bit extended float and push
                    (0xDB, 5) => {
                        let mut bytes = [0u8; 10];
                        for (i, b) in bytes.iter_mut().enumerate() {
                            *b = bus.memory_read_u8(addr + i);
                        }
                        self.fpu_push(F80::from_bytes(bytes));
                    }
                    // FSTP m80 (DB /7): store 80-bit extended float and pop
                    (0xDB, 7) => {
                        let bytes = self.fpu_stack[self.fpu_top as usize].to_bytes();
                        for (i, &b) in bytes.iter().enumerate() {
                            bus.memory_write_u8(addr + i, b);
                        }
                        self.fpu_pop();
                    }
                    // FILD m64 (DF /5): load 64-bit signed integer exactly and push
                    (0xDF, 5) => {
                        let w0 = bus.memory_read_u16(addr) as u64;
                        let w1 = bus.memory_read_u16(addr + 2) as u64;
                        let w2 = bus.memory_read_u16(addr + 4) as u64;
                        let w3 = bus.memory_read_u16(addr + 6) as u64;
                        let bits = w0 | (w1 << 16) | (w2 << 32) | (w3 << 48);
                        self.fpu_push(F80::from_i64(bits as i64));
                    }
                    // FISTP m64 (DF /7): store ST(0) as 64-bit signed integer and pop
                    (0xDF, 7) => {
                        let i = self.fpu_stack[self.fpu_top as usize].to_i64(rc) as u64;
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

        bus.increment_cycle_count(timing::cycles::ESC);
    }
}
