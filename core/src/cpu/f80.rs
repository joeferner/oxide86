/// 80-bit extended-precision float matching the Intel 8087/x87 internal format.
///
/// Wire layout (10 bytes, little-endian):
///   bytes 0–7 : 64-bit significand; bit 63 is the explicit integer bit for normal numbers
///   bytes 8–9 : sign (bit 15) + 15-bit biased exponent (bias = 16383)
///
/// Special encodings:
///   Zero      : exp = 0, mant = 0
///   Denormal  : exp = 0, mant ≠ 0
///   Infinity  : exp = 0x7FFF, mant = 0x8000_0000_0000_0000
///   NaN       : exp = 0x7FFF, mant has any non-zero fraction bits
///   Normal    : exp in 1..0x7FFE, bit 63 of mant set
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct F80 {
    pub sign: bool,
    /// Biased exponent (bias = 16383). 0 = zero/denormal, 0x7FFF = infinity/NaN.
    pub exp: u16,
    /// 64-bit significand. For normal numbers bit 63 is the explicit integer bit.
    pub mant: u64,
}

impl F80 {
    pub const ZERO: F80 = F80 {
        sign: false,
        exp: 0,
        mant: 0,
    };
    pub const NEG_ZERO: F80 = F80 {
        sign: true,
        exp: 0,
        mant: 0,
    };
    pub const ONE: F80 = F80 {
        sign: false,
        exp: 0x3FFF,
        mant: 0x8000_0000_0000_0000,
    };
    pub const POS_INF: F80 = F80 {
        sign: false,
        exp: 0x7FFF,
        mant: 0x8000_0000_0000_0000,
    };
    pub const NEG_INF: F80 = F80 {
        sign: true,
        exp: 0x7FFF,
        mant: 0x8000_0000_0000_0000,
    };
    pub const NAN: F80 = F80 {
        sign: false,
        exp: 0x7FFF,
        mant: 0xC000_0000_0000_0000,
    };

    // 8087-exact constants (from Intel manuals)
    pub const PI: F80 = F80 {
        sign: false,
        exp: 0x4000,
        mant: 0xC90F_DAA2_2168_C235,
    };
    pub const LOG2_E: F80 = F80 {
        sign: false,
        exp: 0x3FFF,
        mant: 0xB8AA_3B29_5C17_F0BC,
    };
    pub const LN_2: F80 = F80 {
        sign: false,
        exp: 0x3FFE,
        mant: 0xB172_17F7_D1CF_79AC,
    };
    pub const LOG2_10: F80 = F80 {
        sign: false,
        exp: 0x4000,
        mant: 0xD49A_784B_CD1B_8AFE,
    };
    pub const LOG10_2: F80 = F80 {
        sign: false,
        exp: 0x3FFD,
        mant: 0x9A20_9A84_FBCF_F799,
    };

    // ── predicates ───────────────────────────────────────────────────────────

    pub fn is_nan(self) -> bool {
        self.exp == 0x7FFF && (self.mant & 0x7FFF_FFFF_FFFF_FFFF) != 0
    }

    pub fn is_infinite(self) -> bool {
        self.exp == 0x7FFF && self.mant == 0x8000_0000_0000_0000
    }

    pub fn is_zero(self) -> bool {
        self.exp == 0 && self.mant == 0
    }

    pub fn is_negative(self) -> bool {
        self.sign
    }

    // ── sign ops ─────────────────────────────────────────────────────────────

    pub fn neg(self) -> F80 {
        F80 {
            sign: !self.sign,
            ..self
        }
    }

    pub fn abs(self) -> F80 {
        F80 {
            sign: false,
            ..self
        }
    }

    // ── conversions ───────────────────────────────────────────────────────────

    /// Convert a signed 64-bit integer to F80 — exact for all i64 values.
    pub fn from_i64(n: i64) -> F80 {
        if n == 0 {
            return F80::ZERO;
        }
        let sign = n < 0;
        // Careful: i64::MIN.wrapping_neg() overflows, use unsigned_abs
        let abs_n = n.unsigned_abs();
        let leading = abs_n.leading_zeros();
        let k = 63 - leading; // position of the highest set bit (0-indexed)
        let exp = 16383 + k as u16;
        // Shift so integer bit sits at bit 63
        let mant = abs_n << (63 - k);
        F80 { sign, exp, mant }
    }

    /// Convert F80 to i64, rounding with the FPU rounding mode (RC field of CW).
    /// rc: 0=nearest(even), 1=floor, 2=ceil, 3=truncate
    pub fn to_i64(self, rc: u8) -> i64 {
        if self.is_nan() || self.is_infinite() || self.is_zero() {
            return 0;
        }
        if self.exp < 16383 {
            // |value| < 1 — round according to mode
            return match rc {
                1 => {
                    if self.sign { -1 } else { 0 } // floor
                }
                2 => {
                    if self.sign { 0 } else { 1 } // ceil
                }
                _ => 0, // nearest / truncate: rounds to 0
            };
        }
        let k = (self.exp - 16383) as u32; // value = mant * 2^(k-63)
        if k >= 63 {
            // |value| >= 2^63
            if k == 63 && self.sign && self.mant == 0x8000_0000_0000_0000 {
                return i64::MIN; // exact -2^63
            }
            return if self.sign { i64::MIN } else { i64::MAX };
        }
        let shift = 63 - k;
        let int_part = (self.mant >> shift) as i64;
        // Round using the remaining fraction bits
        let round = round_i64(self.sign, int_part, self.mant, shift, rc);
        if self.sign { -round } else { round }
    }

    /// Convert f64 → F80 (preserves the f64's value, no precision added).
    pub fn from_f64(v: f64) -> F80 {
        let bits = v.to_bits();
        let sign = (bits >> 63) != 0;
        let exp64 = ((bits >> 52) & 0x7FF) as u32;
        let mant64 = bits & 0x000F_FFFF_FFFF_FFFF;

        if exp64 == 0 && mant64 == 0 {
            return F80 {
                sign,
                exp: 0,
                mant: 0,
            };
        }
        if exp64 == 0x7FF {
            if mant64 == 0 {
                return if sign { F80::NEG_INF } else { F80::POS_INF };
            }
            return F80::NAN;
        }
        if exp64 == 0 {
            // Denormal f64
            return F80 {
                sign,
                exp: 0,
                mant: mant64 << 11,
            };
        }
        let exp80 = (exp64 as i32 - 1023 + 16383) as u16;
        let mant80 = 0x8000_0000_0000_0000u64 | (mant64 << 11);
        F80 {
            sign,
            exp: exp80,
            mant: mant80,
        }
    }

    /// Convert F80 → f64 (may lose precision).
    pub fn to_f64(self) -> f64 {
        if self.is_zero() {
            return if self.sign { -0.0f64 } else { 0.0f64 };
        }
        if self.is_infinite() {
            return if self.sign {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        if self.is_nan() {
            return f64::NAN;
        }
        if self.exp == 0 {
            return if self.sign { -0.0f64 } else { 0.0f64 };
        }
        let exp64 = self.exp as i32 - 16383 + 1023;
        if exp64 <= 0 {
            return if self.sign { -0.0f64 } else { 0.0f64 };
        }
        if exp64 >= 0x7FF {
            return if self.sign {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        // Round to nearest, ties to even (so constants like LOG10_2 convert correctly)
        let shifted = self.mant >> 11;
        let round_bit = (self.mant >> 10) & 1;
        let sticky = (self.mant & ((1u64 << 10) - 1)) != 0;
        let should_round_up = round_bit == 1 && (sticky || (shifted & 1) != 0);
        let rounded = if should_round_up {
            shifted + 1
        } else {
            shifted
        };
        // A carry out of bit 52 means the mantissa overflowed — bump the exponent
        let (exp64, mant64) = if rounded >> 52 > 1 {
            (exp64 + 1, (rounded >> 1) & 0x000F_FFFF_FFFF_FFFF)
        } else {
            (exp64, rounded & 0x000F_FFFF_FFFF_FFFF)
        };
        if exp64 >= 0x7FF {
            return if self.sign {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        let bits = ((self.sign as u64) << 63) | ((exp64 as u64) << 52) | mant64;
        f64::from_bits(bits)
    }

    /// Load from 10-byte x87 little-endian wire format.
    pub fn from_bytes(bytes: [u8; 10]) -> F80 {
        let mant = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let exp_sign = u16::from_le_bytes([bytes[8], bytes[9]]);
        let sign = (exp_sign >> 15) != 0;
        let exp = exp_sign & 0x7FFF;
        F80 { sign, exp, mant }
    }

    /// Store to 10-byte x87 little-endian wire format.
    pub fn to_bytes(self) -> [u8; 10] {
        let mut result = [0u8; 10];
        result[0..8].copy_from_slice(&self.mant.to_le_bytes());
        let exp_sign = self.exp | (if self.sign { 0x8000 } else { 0 });
        result[8..10].copy_from_slice(&exp_sign.to_le_bytes());
        result
    }

    // ── arithmetic ───────────────────────────────────────────────────────────

    /// Add two F80 values with 80-bit precision.
    pub fn add(self, other: F80) -> F80 {
        if self.is_nan() || other.is_nan() {
            return F80::NAN;
        }
        if self.is_infinite() {
            if other.is_infinite() && self.sign != other.sign {
                return F80::NAN; // +inf + -inf
            }
            return self;
        }
        if other.is_infinite() {
            return other;
        }
        if self.is_zero() {
            return other;
        }
        if other.is_zero() {
            return self;
        }

        if self.sign == other.sign {
            add_magnitudes(self.sign, self.exp, self.mant, other.exp, other.mant)
        } else {
            sub_magnitudes(self, other)
        }
    }

    pub fn sub(self, other: F80) -> F80 {
        self.add(other.neg())
    }

    /// Multiply two F80 values with 80-bit precision.
    pub fn mul(self, other: F80) -> F80 {
        if self.is_nan() || other.is_nan() {
            return F80::NAN;
        }
        let result_sign = self.sign ^ other.sign;
        if self.is_infinite() || other.is_infinite() {
            if self.is_zero() || other.is_zero() {
                return F80::NAN; // 0 * inf
            }
            return if result_sign {
                F80::NEG_INF
            } else {
                F80::POS_INF
            };
        }
        if self.is_zero() || other.is_zero() {
            return if result_sign {
                F80::NEG_ZERO
            } else {
                F80::ZERO
            };
        }
        if self.exp == 0 || other.exp == 0 {
            // denormal × anything: approximate via f64
            return F80::from_f64(self.to_f64() * other.to_f64());
        }

        // Both normal: mantissas in [2^63, 2^64-1]; product in [2^126, 2^128-1]
        let prod = (self.mant as u128) * (other.mant as u128);
        let exp_sum = self.exp as i32 + other.exp as i32 - 16383;

        // Determine normalization: if bit 127 set, product ≥ 2^127, shift right 64
        let (mant, exp_adj) = if prod >> 127 != 0 {
            ((prod >> 64) as u64, 1i32)
        } else {
            // bit 126 set, product in [2^126, 2^127-1], shift right 63
            ((prod >> 63) as u64, 0i32)
        };

        let exp_result = exp_sum + exp_adj;
        if exp_result <= 0 {
            return if result_sign {
                F80::NEG_ZERO
            } else {
                F80::ZERO
            };
        }
        if exp_result >= 0x7FFF {
            return if result_sign {
                F80::NEG_INF
            } else {
                F80::POS_INF
            };
        }
        F80 {
            sign: result_sign,
            exp: exp_result as u16,
            mant,
        }
    }

    /// Divide self / other.
    pub fn div(self, other: F80) -> F80 {
        if self.is_nan() || other.is_nan() {
            return F80::NAN;
        }
        let result_sign = self.sign ^ other.sign;
        if other.is_zero() {
            if self.is_zero() {
                return F80::NAN;
            }
            return if result_sign {
                F80::NEG_INF
            } else {
                F80::POS_INF
            };
        }
        if self.is_zero() {
            return if result_sign {
                F80::NEG_ZERO
            } else {
                F80::ZERO
            };
        }
        if self.is_infinite() {
            if other.is_infinite() {
                return F80::NAN;
            }
            return if result_sign {
                F80::NEG_INF
            } else {
                F80::POS_INF
            };
        }
        if other.is_infinite() {
            return if result_sign {
                F80::NEG_ZERO
            } else {
                F80::ZERO
            };
        }
        // Approximate via f64 (sufficient for most use cases; transcendentals use this too)
        let mut result = F80::from_f64(self.to_f64() / other.to_f64());
        result.sign = result_sign;
        result
    }

    // ── rounding ──────────────────────────────────────────────────────────────

    /// Round to integer in place (FRNDINT). rc: 0=nearest-even, 1=floor, 2=ceil, 3=trunc.
    pub fn round_to_integer(self, rc: u8) -> F80 {
        if self.is_nan() || self.is_infinite() || self.is_zero() {
            return self;
        }
        // If exponent already implies an integer (value ≥ 2^63), no rounding needed
        if self.exp >= 16383 + 63 {
            return self;
        }
        let i = self.to_i64(rc);
        F80::from_i64(i)
    }

    // ── comparison ────────────────────────────────────────────────────────────

    /// Compare self vs other, returning (C0, C2, C3) for FCOM/FTST.
    /// unordered → C0=1 C2=1 C3=1
    /// self < other → C0=1 C2=0 C3=0
    /// self > other → C0=0 C2=0 C3=0
    /// self == other → C0=0 C2=0 C3=1
    pub fn compare_cc(self, other: F80) -> (bool, bool, bool) {
        if self.is_nan() || other.is_nan() {
            return (true, true, true);
        }
        // ±0 == ±0
        if self.is_zero() && other.is_zero() {
            return (false, false, true);
        }
        // Signs differ
        if self.sign != other.sign {
            // negative < positive
            let self_lt = self.sign;
            return if self_lt {
                (true, false, false)
            } else {
                (false, false, false)
            };
        }
        // Same sign: compare magnitudes
        let mag_ord = if self.exp != other.exp {
            self.exp.cmp(&other.exp)
        } else {
            self.mant.cmp(&other.mant)
        };
        use std::cmp::Ordering::*;
        let (less, equal) = match mag_ord {
            Less => (true, false),
            Equal => (false, true),
            Greater => (false, false),
        };
        // If negative, reverse the order
        let (less, equal) = if self.sign {
            (!less && !equal, equal)
        } else {
            (less, equal)
        };
        if equal {
            (false, false, true)
        } else if less {
            (true, false, false)
        } else {
            (false, false, false)
        }
    }

    pub fn sqrt(self) -> F80 {
        F80::from_f64(self.to_f64().sqrt())
    }
}

// ── internal helpers ──────────────────────────────────────────────────────────

/// Add two positive magnitudes (same sign `sign`).
fn add_magnitudes(sign: bool, exp_a: u16, mant_a: u64, exp_b: u16, mant_b: u64) -> F80 {
    // Put larger exponent first
    let (large_exp, large_mant, small_exp, small_mant) = if exp_a >= exp_b {
        (exp_a, mant_a, exp_b, mant_b)
    } else {
        (exp_b, mant_b, exp_a, mant_a)
    };

    // Align small into u128
    let shift = (large_exp - small_exp) as u32;
    let small_u128: u128 = if shift >= 64 {
        0
    } else {
        (small_mant as u128) >> shift
    };
    let sum = large_mant as u128 + small_u128;

    if sum >> 64 != 0 {
        // Carry out: shift right 1, bump exponent
        let new_exp = large_exp + 1;
        if new_exp >= 0x7FFF {
            return if sign { F80::NEG_INF } else { F80::POS_INF };
        }
        let new_mant = (sum >> 1) as u64;
        F80 {
            sign,
            exp: new_exp,
            mant: new_mant,
        }
    } else {
        let mant = sum as u64;
        if mant == 0 {
            return F80::ZERO;
        }
        // Normalize
        let leading = mant.leading_zeros();
        let exp_norm = large_exp as i32 - leading as i32;
        if exp_norm <= 0 {
            return F80 {
                sign,
                exp: 0,
                mant: mant << large_exp,
            };
        }
        F80 {
            sign,
            exp: exp_norm as u16,
            mant: mant << leading,
        }
    }
}

/// Subtract magnitudes: compute self + other where they have opposite signs.
fn sub_magnitudes(a: F80, b: F80) -> F80 {
    // Determine which has the larger magnitude
    let (larger, smaller) = {
        let a_bigger = if a.exp != b.exp {
            a.exp > b.exp
        } else {
            a.mant >= b.mant
        };
        if a_bigger { (a, b) } else { (b, a) }
    };
    let result_sign = larger.sign;

    let shift = (larger.exp - smaller.exp) as u32;
    // For the sticky/guard bits we use u128 to avoid losing precision during alignment
    let smaller_mant_shifted: u64 = if shift >= 64 {
        0
    } else {
        smaller.mant >> shift
    };

    let diff = larger.mant.wrapping_sub(smaller_mant_shifted);
    if diff == 0 {
        return F80::ZERO;
    }

    // Normalize
    let leading = diff.leading_zeros();
    let mant_norm = diff << leading;
    let exp_norm = larger.exp as i32 - leading as i32;
    if exp_norm <= 0 {
        // Denormal result
        let shift_back = (-exp_norm) as u32;
        return F80 {
            sign: result_sign,
            exp: 0,
            mant: mant_norm >> shift_back,
        };
    }
    F80 {
        sign: result_sign,
        exp: exp_norm as u16,
        mant: mant_norm,
    }
}

/// Compute the rounded integer magnitude from a fractional F80 and return it as i64 ≥ 0.
/// `shift` is the number of low bits of `mant` that are fractional.
fn round_i64(sign: bool, int_part: i64, mant: u64, shift: u32, rc: u8) -> i64 {
    debug_assert!(shift > 0 && shift < 64);
    let frac_mask = (1u64 << shift) - 1;
    let frac = mant & frac_mask;
    let half = 1u64 << (shift - 1);

    match rc {
        0 => {
            // Round to nearest, ties to even
            if frac > half || (frac == half && (int_part & 1) != 0) {
                int_part + 1
            } else {
                int_part
            }
        }
        1 => {
            // Floor (round toward -∞)
            if sign && frac != 0 {
                int_part + 1
            } else {
                int_part
            }
        }
        2 => {
            // Ceiling (round toward +∞)
            if !sign && frac != 0 {
                int_part + 1
            } else {
                int_part
            }
        }
        _ => int_part, // Truncate toward zero
    }
}
