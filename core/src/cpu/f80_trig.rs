/// IEEE 754 binary128 soft-float for 8087 FPTAN/FPATAN emulation.
///
/// Algorithm derived from BOCHS FPU emulation by Stanislav Shwartsman (LGPL).
/// Uses float128 polynomial approximation to match 8087 output precision.
use crate::cpu::f80::F80;

// ---------------------------------------------------------------------------
// F128: IEEE 754 binary128 (quad precision)
// hi: sign(1) | biased_exp(15) | frac_hi(48)
// lo: frac_lo(64)
// Bias = 16383.  For normal numbers the implicit leading 1 is NOT stored.
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug)]
struct F128 {
    hi: u64,
    lo: u64,
}

impl F128 {
    fn sign(self) -> bool {
        self.hi >> 63 != 0
    }
    fn biased_exp(self) -> i32 {
        ((self.hi >> 48) & 0x7FFF) as i32
    }
    fn frac_hi(self) -> u64 {
        self.hi & 0x0000_FFFF_FFFF_FFFF
    }
    /// Full 113-bit significand (bit 112 = implicit leading 1 for normal numbers).
    /// For zero/denormal (biased_exp == 0) there is no implicit leading 1.
    fn sig(self) -> u128 {
        let frac = ((self.frac_hi() as u128) << 64) | (self.lo as u128);
        if self.biased_exp() == 0 {
            frac
        } else {
            (1u128 << 112) | frac
        }
    }
    fn negate(self) -> F128 {
        F128 {
            hi: self.hi ^ (1 << 63),
            lo: self.lo,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal packing helpers
// ---------------------------------------------------------------------------

/// Pack sign, biased exponent, and 113-bit significand (bit 112 = leading 1)
/// into F128 without rounding (the sig must already be rounded).
/// Applies Bochs-style truncation (zeroes lower 32 bits of frac_lo) to match
/// `softfloat_roundPackToF128` — used by mul, add, div.
fn pack_f128(sign: bool, exp: i32, sig: u128) -> F128 {
    let frac = sig & ((1u128 << 112) - 1); // strip leading 1
    let frac_hi = ((frac >> 64) as u64) & 0x0000_FFFF_FFFF_FFFF;
    // Truncate frac_lo to match x87 hardware's ~67-bit internal significand.
    // Bochs uses 0xFFFFFFFF00000000 (~80 bits) as an approximation but
    // 0xFFFFC00000000000 (~67 bits) better matches real 8087 polynomial output.
    let frac_lo = (frac as u64) & 0xFFFF_C000_0000_0000;
    let hi = ((sign as u64) << 63) | ((exp as u64 & 0x7FFF) << 48) | frac_hi;
    F128 { hi, lo: frac_lo }
}

/// Pack without truncation — matches Bochs's `softfloat_normRoundPackToF128`
/// fast path used by subtraction, which preserves full sig0 precision.
fn pack_f128_full(sign: bool, exp: i32, sig: u128) -> F128 {
    let frac = sig & ((1u128 << 112) - 1); // strip leading 1
    let frac_hi = ((frac >> 64) as u64) & 0x0000_FFFF_FFFF_FFFF;
    let frac_lo = frac as u64; // NO truncation
    let hi = ((sign as u64) << 63) | ((exp as u64 & 0x7FFF) << 48) | frac_hi;
    F128 { hi, lo: frac_lo }
}

/// Pack via Bochs-style truncation (no rounding).
/// Bochs's `softfloat_roundPackToF128` zeroes `sigExtra` and masks `sig0`
/// BEFORE the rounding decision, so `doIncrement` is always false.  This
/// means the F128 intermediate results are always truncated, never rounded.
/// The round/sticky parameters are accepted for call-site compatibility but
/// ignored — matching Bochs behaviour.
fn pack_rounded(sign: bool, exp: i32, sig: u128, _round: bool, _sticky: bool) -> F128 {
    pack_f128(sign, exp, sig)
}

// ---------------------------------------------------------------------------
// Shift-right-jam: shift a 113-bit sig right by `n`, collect round+sticky.
// Returns (shifted_sig, round_bit, sticky_bit).
// ---------------------------------------------------------------------------
fn shift_right_jam(sig: u128, n: u32) -> (u128, bool, bool) {
    if n == 0 {
        return (sig, false, false);
    }
    if n >= 128 {
        return (0, false, sig != 0);
    }
    if n > 113 {
        let round = if n <= 114 {
            (sig >> (n - 1)) & 1 != 0
        } else {
            false
        };
        let sticky = if n == 114 {
            (sig & ((1u128 << 113) - 1)) != 0
        } else {
            sig != 0
        };
        return (0, round, sticky);
    }
    let shifted = sig >> n;
    let round = (sig >> (n - 1)) & 1 != 0;
    let sticky = n >= 2 && (sig & ((1u128 << (n - 1)) - 1)) != 0;
    (shifted, round, sticky)
}

// ---------------------------------------------------------------------------
// 128×128 → 256 multiplication (only needs top ~226 bits; both inputs ≤113 bits)
// Returns (hi128, lo128) where the full product = hi128*2^128 + lo128.
// ---------------------------------------------------------------------------
fn mul128(a: u128, b: u128) -> (u128, u128) {
    let a0 = a & 0xFFFF_FFFF_FFFF_FFFF;
    let a1 = a >> 64;
    let b0 = b & 0xFFFF_FFFF_FFFF_FFFF;
    let b1 = b >> 64;

    let p00 = a0 * b0;
    let p01 = a0 * b1;
    let p10 = a1 * b0;
    let p11 = a1 * b1;

    let p00_lo = p00 & 0xFFFF_FFFF_FFFF_FFFF;
    let p00_hi = p00 >> 64;

    let mid = p01 + p10; // ≤114 bits for 113-bit inputs
    let mid_lo = mid & 0xFFFF_FFFF_FFFF_FFFF;
    let mid_hi = mid >> 64;

    let lo_hi_sum = p00_hi + mid_lo;
    let carry = lo_hi_sum >> 64;
    let lo = p00_lo | ((lo_hi_sum & 0xFFFF_FFFF_FFFF_FFFF) << 64);
    let hi = p11 + mid_hi + carry;
    (hi, lo)
}

// ---------------------------------------------------------------------------
// Float128 arithmetic (normalized inputs only; no NaN/Inf/denormal handling)
// ---------------------------------------------------------------------------

fn f128_mul(a: F128, b: F128) -> F128 {
    let sign = a.sign() ^ b.sign();
    let sig_a = a.sig();
    let sig_b = b.sig();
    if sig_a == 0 || sig_b == 0 {
        return F128 {
            hi: (sign as u64) << 63,
            lo: 0,
        };
    }
    let exp_a = a.biased_exp();
    let exp_b = b.biased_exp();

    let (hi, lo) = mul128(sig_a, sig_b);
    // Product of two 113-bit values has MSB at bit 224 or 225 (= bit 96 or 97 of hi).
    // In (hi, lo): product[225] = bit 97 of hi, product[224] = bit 96 of hi.
    let exp_z = exp_a + exp_b - 16383;

    let (exp_z, sig, round, sticky) = if (hi >> 97) & 1 != 0 {
        // MSB at product bit 225 → result needs shift right by 1
        let sig = ((hi & ((1u128 << 98) - 1)) << 15) | (lo >> 113);
        let round = (lo >> 112) & 1 != 0;
        let sticky = (lo & ((1u128 << 112) - 1)) != 0;
        (exp_z + 1, sig, round, sticky)
    } else {
        // MSB at product bit 224
        let sig = ((hi & ((1u128 << 97) - 1)) << 16) | (lo >> 112);
        let round = (lo >> 111) & 1 != 0;
        let sticky = (lo & ((1u128 << 111) - 1)) != 0;
        (exp_z, sig, round, sticky)
    };

    pack_rounded(sign, exp_z, sig, round, sticky)
}

fn f128_add_mags(a: F128, b: F128, sign: bool) -> F128 {
    let exp_a = a.biased_exp();
    let exp_b = b.biased_exp();

    let (exp_l, sig_l, exp_s, sig_s) = if exp_a >= exp_b {
        (exp_a, a.sig(), exp_b, b.sig())
    } else {
        (exp_b, b.sig(), exp_a, a.sig())
    };

    let shift = (exp_l - exp_s) as u32;
    let (sig_s_aligned, round, sticky) = shift_right_jam(sig_s, shift);

    let sum = sig_l + sig_s_aligned; // at most 114 bits

    if sum >> 113 != 0 {
        // overflow: shift right 1; old bit-0 → new round; old round → sticky
        let new_round = (sum & 1) != 0;
        let new_sticky = round | sticky;
        let sig_z = sum >> 1;
        pack_rounded(sign, exp_l + 1, sig_z, new_round, new_sticky)
    } else {
        pack_rounded(sign, exp_l, sum, round, sticky)
    }
}

fn f128_sub_mags(a: F128, b: F128, sign: bool) -> F128 {
    // |a| >= |b| guaranteed by caller — result is positive, sign is from a.
    let exp_a = a.biased_exp();
    let exp_b = b.biased_exp();

    // Shift b sig right to align exponents; no rounding needed (subtraction
    // of smaller from larger can only make result exact or lose low bits).
    let sig_a = a.sig();
    let sig_b_aligned = if exp_a > exp_b {
        let shift = (exp_a - exp_b) as u32;
        // For subtraction we just jam — the lost bits don't round up.
        if shift >= 113 {
            0u128
        } else {
            b.sig() >> shift
        }
    } else {
        b.sig()
    };

    let diff = sig_a.wrapping_sub(sig_b_aligned); // sig_a >= sig_b_aligned since |a|>=|b|

    if diff == 0 {
        return F128 { hi: 0, lo: 0 }; // exact zero
    }

    // Normalize: count leading zeros above bit 112
    let lz = diff.leading_zeros(); // leading zeros in 128-bit value
    // bit 112 should be the MSB after normalization
    // diff has MSB at position (127 - lz)
    let msb_pos = 127 - lz; // bit position of MSB in diff
    let sig_z;
    let exp_z;
    if msb_pos >= 112 {
        let shift = msb_pos - 112;
        sig_z = diff >> shift;
        exp_z = exp_a - (shift as i32);
    } else {
        let shift = 112 - msb_pos;
        sig_z = diff << shift;
        exp_z = exp_a - (shift as i32);
    }

    // Bochs subtraction uses normRoundPackToF128 fast path which does NOT
    // truncate the lower 32 bits of sig0.  Use pack_f128_full to match.
    pack_f128_full(sign, exp_z, sig_z)
}

fn f128_add(a: F128, b: F128) -> F128 {
    let sign_a = a.sign();
    let sign_b = b.sign();
    if sign_a == sign_b {
        f128_add_mags(a, b, sign_a)
    } else {
        // Different signs → subtraction; determine which has larger magnitude
        let exp_a = a.biased_exp();
        let exp_b = b.biased_exp();
        let a_bigger = if exp_a != exp_b {
            exp_a > exp_b
        } else {
            a.sig() >= b.sig()
        };
        if a_bigger {
            f128_sub_mags(a, b, sign_a)
        } else {
            f128_sub_mags(b, a, sign_b)
        }
    }
}

fn f128_sub(a: F128, b: F128) -> F128 {
    f128_add(a, b.negate())
}

/// 113-bit long-division: compute floor(sig_a * 2^113 / sig_b).
/// Returns (quotient_114bits, round, sticky).
/// quotient has bit 113 set when sig_a >= sig_b.
fn div_sig(sig_a: u128, sig_b: u128) -> (u128, bool, bool) {
    let (overflow, mut r) = if sig_a >= sig_b {
        (true, sig_a - sig_b)
    } else {
        (false, sig_a)
    };

    let mut q = 0u128;
    // 113 binary long-division steps for the fractional bits [112:0]
    for bit in (0u32..113).rev() {
        r <<= 1; // r < sig_b < 2^113, so r*2 < 2^114, fits in u128
        if r >= sig_b {
            r -= sig_b;
            q |= 1u128 << bit;
        }
    }

    let q = if overflow { q | (1u128 << 113) } else { q };

    let two_r = r << 1; // r < sig_b < 2^113 → fits in u128
    let round = two_r >= sig_b;
    let sticky = two_r > sig_b;
    (q, round, sticky)
}

fn f128_div(a: F128, b: F128) -> F128 {
    let sign = a.sign() ^ b.sign();
    let sig_a = a.sig();
    if sig_a == 0 {
        return F128 {
            hi: (sign as u64) << 63,
            lo: 0,
        };
    }
    let exp_a = a.biased_exp();
    let exp_b = b.biased_exp();
    let sig_b = b.sig();

    // exp_z = exp_a - exp_b + 16382:
    // - If sig_a >= sig_b (quotient in [1,2)): overflow branch adds 1 → biased_exp = exp_a - exp_b + 16383 ✓
    // - If sig_a < sig_b  (quotient in [0.5,1)): no overflow → biased_exp = exp_a - exp_b + 16382 ✓
    let exp_z = exp_a - exp_b + 16382;
    let (q, round, sticky) = div_sig(sig_a, sig_b);

    let (exp_z, sig_z, round, sticky) = if q >> 113 != 0 {
        let new_sticky = round | sticky;
        let new_round = (q & 1) != 0;
        (exp_z + 1, q >> 1, new_round, new_sticky)
    } else {
        (exp_z, q, round, sticky)
    };

    pack_rounded(sign, exp_z, sig_z, round, sticky)
}

/// Fused multiply-add: round(a*b + c) with a single rounding.
/// We approximate it by: compute a*b to full precision (226 bits),
/// then add c, then round.  This matches softfloat for well-conditioned inputs.
fn f128_muladd(a: F128, b: F128, c: F128) -> F128 {
    // If either multiplicand is zero the product is zero → result is just c.
    if a.sig() == 0 || b.sig() == 0 {
        return c;
    }

    // Step 1: exact 226-bit product of a and b
    let sign_ab = a.sign() ^ b.sign();
    let exp_ab_raw = a.biased_exp() + b.biased_exp() - 16383;
    let sig_a = a.sig();
    let sig_b = b.sig();

    let (prod_hi, prod_lo) = mul128(sig_a, sig_b);

    // Determine the MSB position of the product
    let (exp_ab, prod_hi, prod_lo) = if (prod_hi >> 97) & 1 != 0 {
        (exp_ab_raw + 1, prod_hi, prod_lo)
    } else {
        (exp_ab_raw, prod_hi, prod_lo)
    };

    // Extract the 113-bit product sig and the extra bits for precision
    // We'll keep 113+extra bits of the product during the add.
    // Use 64 extra bits: represent product as (sig_113: u128, extra: u128)
    // where the extra holds the lower bits.
    let (prod_sig, prod_extra) = if (prod_hi >> 97) & 1 != 0 {
        // MSB at product bit 225
        let sig = ((prod_hi & ((1u128 << 98) - 1)) << 15) | (prod_lo >> 113);
        let extra = prod_lo << 15; // bits shifted out, into a 128-bit accumulator
        (sig, extra)
    } else {
        let sig = ((prod_hi & ((1u128 << 97) - 1)) << 16) | (prod_lo >> 112);
        let extra = prod_lo << 16;
        (sig, extra)
    };

    // Step 2: Add c to the product.
    // If signs match, add; otherwise subtract.
    let sign_c = c.sign();
    let exp_c = c.biased_exp();
    let sig_c = c.sig();

    if sign_ab == sign_c {
        // Addition: align c to the product's exponent
        let exp_diff = exp_ab - exp_c;
        let (sig_c_aligned, c_extra, round, sticky) = if exp_diff >= 226 {
            // c is negligible
            let sticky = sig_c != 0;
            (0u128, 0u128, false, sticky)
        } else if exp_diff >= 128 {
            let shift = exp_diff as u32 - 128;
            let c_hi = if shift >= 113 { 0 } else { sig_c >> shift };
            let c_extra_bits = if shift < 113 {
                sig_c << (113 - shift)
            } else {
                0
            };
            (
                c_hi,
                c_extra_bits,
                false,
                c_hi == 0 && c_extra_bits == 0 && sig_c != 0,
            )
        } else if exp_diff >= 0 {
            let shift = exp_diff as u32;
            let (aligned, r, s) = shift_right_jam(sig_c, shift);
            (aligned, 0u128, r, s)
        } else {
            // c has larger exponent — add c to a smaller product
            // Swap: result dominated by c.  Shift product to align with c.
            let shift = (-exp_diff) as u32;
            let (prod_aligned, r, s) = shift_right_jam(prod_sig, shift);
            // prod_extra is also shifted — simplified: treat as sticky
            let sticky2 = s | (prod_extra != 0);
            let sum = sig_c + prod_aligned;
            let (exp_z, sig_z, round, sticky) = if sum >> 113 != 0 {
                (exp_c + 1, sum >> 1, (sum & 1) != 0, r | sticky2)
            } else {
                (exp_c, sum, r, sticky2)
            };
            return pack_rounded(sign_ab, exp_z, sig_z, round, sticky);
        };
        // sum of (prod_sig, prod_extra) + (sig_c_aligned, c_extra)
        let extra_sum = prod_extra.wrapping_add(c_extra);
        let carry = if extra_sum < prod_extra { 1u128 } else { 0u128 };
        let sig_sum = prod_sig + sig_c_aligned + carry;
        let round_from_extra = (extra_sum >> 127) != 0;
        let sticky_extra = (extra_sum & ((1u128 << 127) - 1)) != 0 || round || sticky;
        let (exp_z, sig_z, r, s) = if sig_sum >> 113 != 0 {
            let s2 = sticky_extra | round_from_extra | ((sig_sum & 1) != 0);
            (exp_ab + 1, sig_sum >> 1, (sig_sum & 1) != 0, s2)
        } else {
            (exp_ab, sig_sum, round_from_extra, sticky_extra)
        };
        pack_rounded(sign_ab, exp_z, sig_z, r, s)
    } else {
        // Subtraction: fused operation — subtract c from product (or vice versa)
        // without intermediate rounding, matching Bochs's single-rounding f128_mulAdd.
        let exp_diff = exp_ab - exp_c;

        // Determine which magnitude is larger and compute the subtraction.
        // We work with (sig_hi: u128, sig_lo: u128) pairs for extra precision.
        if exp_diff > 0
            || (exp_diff == 0 && (prod_sig > sig_c || (prod_sig == sig_c && prod_extra != 0)))
        {
            // |product| > |c|, result sign = sign_ab
            // Align c to product's exponent by shifting right
            let (c_aligned, c_extra) = if exp_diff >= 226 {
                (0u128, 0u128)
            } else if exp_diff >= 128 {
                let shift = exp_diff as u32 - 128;
                if shift >= 113 {
                    (0, 0)
                } else {
                    (0u128, sig_c >> shift)
                }
            } else if exp_diff > 0 {
                let shift = exp_diff as u32;
                let aligned = sig_c >> shift;
                let extra = sig_c << (128 - shift);
                (aligned, extra)
            } else {
                (sig_c, 0u128)
            };

            // Subtract: (prod_sig, prod_extra) - (c_aligned, c_extra)
            let borrow = if prod_extra < c_extra { 1u128 } else { 0u128 };
            let diff_extra = prod_extra.wrapping_sub(c_extra);
            let diff_sig = prod_sig.wrapping_sub(c_aligned).wrapping_sub(borrow);

            if diff_sig == 0 && diff_extra == 0 {
                return F128 { hi: 0, lo: 0 };
            }

            // Normalize: find MSB position in diff_sig
            if diff_sig == 0 {
                // All significant bits are in diff_extra — large cancellation
                let lz = diff_extra.leading_zeros();
                let shift_up = lz + 128 - 112;
                let sig_z = diff_extra << lz >> (128 - 113);
                let exp_z = exp_ab - shift_up as i32;
                let remaining = diff_extra << (lz + 113);
                let round = (remaining >> 127) != 0;
                let sticky = (remaining & ((1u128 << 127) - 1)) != 0;
                pack_rounded(sign_ab, exp_z, sig_z, round, sticky)
            } else {
                let lz = diff_sig.leading_zeros();
                let msb_pos = 127 - lz;
                if msb_pos >= 112 {
                    let shift = msb_pos - 112;
                    let sig_z = (diff_sig >> shift)
                        | if shift > 0 {
                            diff_extra >> (128 - shift)
                        } else {
                            0
                        };
                    let round = if shift > 0 {
                        (diff_sig >> (shift - 1)) & 1 != 0
                    } else {
                        (diff_extra >> 127) != 0
                    };
                    let sticky = if shift > 1 {
                        (diff_sig & ((1u128 << (shift - 1)) - 1)) != 0 || diff_extra != 0
                    } else if shift == 1 {
                        diff_extra != 0
                    } else {
                        (diff_extra & ((1u128 << 127) - 1)) != 0
                    };
                    let exp_z = exp_ab + (msb_pos as i32 - 112);
                    pack_rounded(sign_ab, exp_z, sig_z, round, sticky)
                } else {
                    let shift = 112 - msb_pos;
                    let sig_z = (diff_sig << shift) | (diff_extra >> (128 - shift));
                    let round = (diff_extra >> (127 - shift)) & 1 != 0;
                    let sticky = (diff_extra & ((1u128 << (127 - shift)) - 1)) != 0;
                    let exp_z = exp_ab - (shift as i32);
                    pack_rounded(sign_ab, exp_z, sig_z, round, sticky)
                }
            }
        } else if exp_diff < 0 || (exp_diff == 0 && sig_c > prod_sig) {
            // |c| > |product|, result sign = sign_c
            // Align product to c's exponent
            let neg_diff = -exp_diff;
            let (prod_aligned, p_extra) = if neg_diff >= 226 {
                (0u128, 0u128)
            } else if neg_diff >= 128 {
                let shift = neg_diff as u32 - 128;
                if shift >= 113 {
                    (0, 0)
                } else {
                    (0u128, prod_sig >> shift)
                }
            } else if neg_diff > 0 {
                let shift = neg_diff as u32;
                let aligned = prod_sig >> shift;
                let extra = (prod_sig << (128 - shift)) | (prod_extra >> shift);
                (aligned, extra)
            } else {
                (prod_sig, prod_extra)
            };

            let borrow = if 0u128 < p_extra { 1u128 } else { 0u128 };
            let diff_extra = 0u128.wrapping_sub(p_extra);
            let diff_sig = sig_c.wrapping_sub(prod_aligned).wrapping_sub(borrow);

            if diff_sig == 0 && diff_extra == 0 {
                return F128 { hi: 0, lo: 0 };
            }

            // Normalize
            if diff_sig == 0 {
                let lz = diff_extra.leading_zeros();
                let shift_up = lz + 128 - 112;
                let sig_z = diff_extra << lz >> (128 - 113);
                let exp_z = exp_c - shift_up as i32;
                let remaining = diff_extra << (lz + 113);
                let round = (remaining >> 127) != 0;
                let sticky = (remaining & ((1u128 << 127) - 1)) != 0;
                pack_rounded(sign_c, exp_z, sig_z, round, sticky)
            } else {
                let lz = diff_sig.leading_zeros();
                let msb_pos = 127 - lz;
                if msb_pos >= 112 {
                    let shift = msb_pos - 112;
                    let sig_z = (diff_sig >> shift)
                        | if shift > 0 {
                            diff_extra >> (128 - shift)
                        } else {
                            0
                        };
                    let round = if shift > 0 {
                        (diff_sig >> (shift - 1)) & 1 != 0
                    } else {
                        (diff_extra >> 127) != 0
                    };
                    let sticky = if shift > 1 {
                        (diff_sig & ((1u128 << (shift - 1)) - 1)) != 0 || diff_extra != 0
                    } else if shift == 1 {
                        diff_extra != 0
                    } else {
                        (diff_extra & ((1u128 << 127) - 1)) != 0
                    };
                    let exp_z = exp_c + (msb_pos as i32 - 112);
                    pack_rounded(sign_c, exp_z, sig_z, round, sticky)
                } else {
                    let shift = 112 - msb_pos;
                    let sig_z = (diff_sig << shift) | (diff_extra >> (128 - shift));
                    let round = (diff_extra >> (127 - shift)) & 1 != 0;
                    let sticky = (diff_extra & ((1u128 << (127 - shift)) - 1)) != 0;
                    let exp_z = exp_c - (shift as i32);
                    pack_rounded(sign_c, exp_z, sig_z, round, sticky)
                }
            }
        } else {
            // Exact cancellation: |product| == |c|
            F128 { hi: 0, lo: 0 }
        }
    }
}

// ---------------------------------------------------------------------------
// Conversions between F80 and F128
// ---------------------------------------------------------------------------

/// Convert a normalized F80 to F128.  The conversion is exact (the 63 fraction
/// bits of the F80 become the top 63 bits of the 112-bit F128 fraction).
fn f80_to_f128(a: F80) -> F128 {
    // F80 mant: bit 63 = explicit integer 1, bits [62:0] = fraction
    // F128 fraction (112 bits): top 63 bits = F80 fraction, bottom 49 bits = 0
    // frac_hi (48 bits) = mant[62:15]
    // frac_lo (64 bits) = mant[14:0] << 49
    let frac_hi = (a.mant >> 15) & 0x0000_FFFF_FFFF_FFFF;
    let frac_lo = (a.mant & 0x7FFF) << 49;
    let hi = ((a.sign as u64) << 63) | ((a.exp as u64) << 48) | frac_hi;
    F128 { hi, lo: frac_lo }
}

/// Convert F128 to F80 by truncation (no rounding).
///
/// The x87 FPU computes trig/atan results using an internal f128-like format
/// with ~80 significant bits (Bochs's `roundPackToF128` truncation).  When
/// storing the result back to the 64-bit-significand F80 stack, truncation
/// rather than round-to-nearest produces results matching real 8087 hardware
/// for polynomial-evaluated functions (FTAN, FPATAN, etc.).
fn f128_to_f80(x: F128) -> F80 {
    // Zero: biased_exp=0 with no fraction bits → return true F80 zero.
    if x.biased_exp() == 0 && x.frac_hi() == 0 && x.lo == 0 {
        return F80 {
            sign: x.sign(),
            exp: 0,
            mant: 0,
        };
    }
    let sign = x.sign();
    let exp = x.biased_exp() as u16;
    let frac_hi = x.frac_hi(); // 48 bits
    let frac_lo = x.lo; // 64 bits

    // Reconstruct 113-bit sig: [1 | frac_hi(48) | frac_lo(64)]
    // Shift left 15 to put implicit 1 at bit 63 of a 64-bit word:
    //   mant = (1 << 63) | (frac_hi << 15) | (frac_lo >> 49)
    let mant = (1u64 << 63) | (frac_hi << 15) | (frac_lo >> 49);

    F80 { sign, exp, mant }
}

// ---------------------------------------------------------------------------
// Polynomial evaluation (Horner's method, matching BOCHS poly.cc)
// eval_poly(x, arr) = arr[n-1]*x^(n-1) + ... + arr[1]*x + arr[0]
// odd_poly(x, arr)  = x * eval_poly(x^2, arr)
// even_poly(x, arr) = eval_poly(x^2, arr)
// ---------------------------------------------------------------------------

fn eval_poly(x: F128, arr: &[F128]) -> F128 {
    let n = arr.len();
    let mut r = arr[n - 1];
    for i in (0..n - 1).rev() {
        r = f128_muladd(r, x, arr[i]);
    }
    r
}

fn odd_poly(x: F128, arr: &[F128]) -> F128 {
    let x2 = f128_mul(x, x);
    f128_mul(x, eval_poly(x2, arr))
}

fn even_poly(x: F128, arr: &[F128]) -> F128 {
    let x2 = f128_mul(x, x);
    eval_poly(x2, arr)
}

// ---------------------------------------------------------------------------
// Constants (from BOCHS fpatan.cc / fsincos.cc)
// ---------------------------------------------------------------------------

const F128_ONE: F128 = F128 {
    hi: 0x3fff000000000000,
    lo: 0x0000000000000000,
};
const F128_SQRT3: F128 = F128 {
    hi: 0x3fffbb67ae8584ca,
    lo: 0xa73b25742d7078b8,
};
const F128_PI2: F128 = F128 {
    hi: 0x3fff921fb54442d1,
    lo: 0x8469898cc5170416,
};
const F128_PI4: F128 = F128 {
    hi: 0x3ffe921fb54442d1,
    lo: 0x8469898cc5170416,
};
const F128_PI6: F128 = F128 {
    hi: 0x3ffe0c152382d736,
    lo: 0x58465bb32e0f580f,
};

const ATAN_ARR: [F128; 11] = [
    F128 {
        hi: 0x3fff000000000000,
        lo: 0x0000000000000000,
    }, /*  1 */
    F128 {
        hi: 0xbffd555555555555,
        lo: 0x5555555555555555,
    }, /*  3 */
    F128 {
        hi: 0x3ffc999999999999,
        lo: 0x999999999999999a,
    }, /*  5 */
    F128 {
        hi: 0xbffc249249249249,
        lo: 0x2492492492492492,
    }, /*  7 */
    F128 {
        hi: 0x3ffbc71c71c71c71,
        lo: 0xc71c71c71c71c71c,
    }, /*  9 */
    F128 {
        hi: 0xbffb745d1745d174,
        lo: 0x5d1745d1745d1746,
    }, /* 11 */
    F128 {
        hi: 0x3ffb3b13b13b13b1,
        lo: 0x3b13b13b13b13b14,
    }, /* 13 */
    F128 {
        hi: 0xbffb111111111111,
        lo: 0x1111111111111111,
    }, /* 15 */
    F128 {
        hi: 0x3ffae1e1e1e1e1e1,
        lo: 0xe1e1e1e1e1e1e1e2,
    }, /* 17 */
    F128 {
        hi: 0xbffaaf286bca1af2,
        lo: 0x86bca1af286bca1b,
    }, /* 19 */
    F128 {
        hi: 0x3ffa861861861861,
        lo: 0x8618618618618618,
    }, /* 21 */
];

const SIN_ARR: [F128; 11] = [
    F128 {
        hi: 0x3fff000000000000,
        lo: 0x0000000000000000,
    }, /*  1 */
    F128 {
        hi: 0xbffc555555555555,
        lo: 0x5555555555555555,
    }, /*  3 */
    F128 {
        hi: 0x3ff8111111111111,
        lo: 0x1111111111111111,
    }, /*  5 */
    F128 {
        hi: 0xbff2a01a01a01a01,
        lo: 0xa01a01a01a01a01a,
    }, /*  7 */
    F128 {
        hi: 0x3fec71de3a556c73,
        lo: 0x38faac1c88e50017,
    }, /*  9 */
    F128 {
        hi: 0xbfe5ae64567f544e,
        lo: 0x38fe747e4b837dc7,
    }, /* 11 */
    F128 {
        hi: 0x3fde6124613a86d0,
        lo: 0x97ca38331d23af68,
    }, /* 13 */
    F128 {
        hi: 0xbfd6ae7f3e733b81,
        lo: 0xf11d8656b0ee8cb0,
    }, /* 15 */
    F128 {
        hi: 0x3fce952c77030ad4,
        lo: 0xa6b2605197771b00,
    }, /* 17 */
    F128 {
        hi: 0xbfc62f49b4681415,
        lo: 0x724ca1ec3b7b9675,
    }, /* 19 */
    F128 {
        hi: 0x3fbd71b8ef6dcf57,
        lo: 0x18bef146fcee6e45,
    }, /* 21 */
];

const COS_ARR: [F128; 11] = [
    F128 {
        hi: 0x3fff000000000000,
        lo: 0x0000000000000000,
    }, /*  0 */
    F128 {
        hi: 0xbffe000000000000,
        lo: 0x0000000000000000,
    }, /*  2 */
    F128 {
        hi: 0x3ffa555555555555,
        lo: 0x5555555555555555,
    }, /*  4 */
    F128 {
        hi: 0xbff56c16c16c16c1,
        lo: 0x6c16c16c16c16c17,
    }, /*  6 */
    F128 {
        hi: 0x3fefa01a01a01a01,
        lo: 0xa01a01a01a01a01a,
    }, /*  8 */
    F128 {
        hi: 0xbfe927e4fb7789f5,
        lo: 0xc72ef016d3ea6679,
    }, /* 10 */
    F128 {
        hi: 0x3fe21eed8eff8d89,
        lo: 0x7b544da987acfe85,
    }, /* 12 */
    F128 {
        hi: 0xbfda93974a8c07c9,
        lo: 0xd20badf145dfa3e5,
    }, /* 14 */
    F128 {
        hi: 0x3fd2ae7f3e733b81,
        lo: 0xf11d8656b0ee8cb0,
    }, /* 16 */
    F128 {
        hi: 0xbfca6827863b97d9,
        lo: 0x77bb004886a2c2ab,
    }, /* 18 */
    F128 {
        hi: 0x3fc1e542ba402022,
        lo: 0x507a9cad2bf8f0bb,
    }, /* 20 */
];

// ---------------------------------------------------------------------------
// FPATAN: atan2(ST(1), ST(0))  —  direct port of BOCHS fpatan.cc
// ---------------------------------------------------------------------------

/// Compute atan2(y, x) where x = ST(0), y = ST(1).
/// Returns atan(y/x) with quadrant correction.
pub fn fpatan(x: F80, y: F80) -> F80 {
    // Handle case |x| == |y| with same exponent/significand
    if x.mant == y.mant && x.exp == y.exp {
        if y.sign {
            // 3π/4
            let r = F80 {
                sign: x.sign,
                exp: 0x4000,
                mant: 0x96CB_E3F9_990E_91A8,
            };
            return r;
        } else {
            // π/4
            return F80 {
                sign: x.sign,
                exp: 0x3FFE,
                mant: 0xC90F_DAA2_2168_C235,
            };
        }
    }

    let x128 = f80_to_f128(F80 { sign: false, ..x });
    let y128 = f80_to_f128(F80 { sign: false, ..y });

    let x_bigger = x.exp > y.exp || (x.exp == y.exp && x.mant > y.mant);

    let (mut arg, swap) = if x_bigger {
        (f128_div(y128, x128), false)
    } else {
        (f128_div(x128, y128), true)
    };

    // Argument reduction: if arg > 3/4, use atan(x) = atan((x-1)/(x+1)) + π/4
    // if arg > 1/4, use atan(x) = atan((x√3 - 1)/(x + √3)) + π/6
    let exp_arg = arg.biased_exp();
    let mut add_pi4 = false;
    let mut add_pi6 = false;

    if exp_arg >= 16383 - 1 && arg.hi >= 0x3FFE_8000_0000_0000 {
        // arg > 3/4 (approximately)
        let t1 = f128_sub(arg, F128_ONE);
        let t2 = f128_add(arg, F128_ONE);
        arg = f128_div(t1, t2);
        add_pi4 = true;
    } else if exp_arg >= 0x3FFD {
        // arg > 1/4
        let t1 = f128_mul(arg, F128_SQRT3);
        let t2 = f128_add(arg, F128_SQRT3);
        let t1 = f128_sub(t1, F128_ONE);
        arg = f128_div(t1, t2);
        add_pi6 = true;
    }

    let mut result = odd_poly(arg, &ATAN_ARR);
    if add_pi6 {
        result = f128_add(result, F128_PI6);
    }
    if add_pi4 {
        result = f128_add(result, F128_PI4);
    }

    if swap {
        result = f128_sub(F128_PI2, result);
    }

    let mut out = f128_to_f80(result);
    // Apply sign: zSign = xSign ^ ySign (from BOCHS)
    let z_sign = x.sign ^ y.sign;
    if z_sign {
        out.sign = !out.sign;
    }
    // Quadrant correction based on bSign (= x.sign in BOCHS convention)
    if !x.sign && out.sign {
        out = f80_add(out, F80::PI);
    } else if x.sign && !out.sign {
        out = f80_sub(out, F80::PI);
    }
    out
}

// F80 add/sub helpers using f64 (sufficient precision for the π correction)
fn f80_add(a: F80, b: F80) -> F80 {
    F80::from_f64(a.to_f64() + b.to_f64())
}
fn f80_sub(a: F80, b: F80) -> F80 {
    F80::from_f64(a.to_f64() - b.to_f64())
}

// ---------------------------------------------------------------------------
// FPTAN argument reduction (ported from BOCHS fsincos.cc)
// ---------------------------------------------------------------------------

const PI_HI: u64 = 0xc90f_daa2_2168_c234;
const PI_LO: u64 = 0xc4c6_628b_80dc_1cd1;

fn shift128_right(hi: u64, lo: u64, n: u32) -> (u64, u64) {
    let v = ((hi as u128) << 64) | (lo as u128);
    let v = v >> n;
    ((v >> 64) as u64, v as u64)
}

fn shift128_left(hi: u64, lo: u64, n: u32) -> (u64, u64) {
    let v = ((hi as u128) << 64) | (lo as u128);
    let v = v << n;
    ((v >> 64) as u64, v as u64)
}

fn add128(ah: u64, al: u64, bh: u64, bl: u64) -> (u64, u64) {
    let a = ((ah as u128) << 64) | (al as u128);
    let b = ((bh as u128) << 64) | (bl as u128);
    let r = a.wrapping_add(b);
    ((r >> 64) as u64, r as u64)
}

fn sub128(ah: u64, al: u64, bh: u64, bl: u64) -> (u64, u64) {
    let a = ((ah as u128) << 64) | (al as u128);
    let b = ((bh as u128) << 64) | (bl as u128);
    let r = a.wrapping_sub(b);
    ((r >> 64) as u64, r as u64)
}

fn lt128(ah: u64, al: u64, bh: u64, bl: u64) -> bool {
    let a = ((ah as u128) << 64) | (al as u128);
    let b = ((bh as u128) << 64) | (bl as u128);
    a < b
}

fn eq128(ah: u64, al: u64, bh: u64, bl: u64) -> bool {
    ah == bh && al == bl
}

/// Estimate (a_hi:a_lo) / b_64 — result within 2 of the true quotient.
/// Requires b_64 >= 2^63.
fn estimate_div128_to64(a0: u64, a1: u64, b: u64) -> u64 {
    if b <= a0 {
        return u64::MAX;
    }
    let b0 = b >> 32;
    let mut z: u64 = if (b0 << 32) <= a0 {
        0xFFFF_FFFF_0000_0000
    } else {
        (a0 / b0) << 32
    };
    // Compute b*z as 128-bit
    let bz = (b as u128) * (z as u128);
    let bz_hi = (bz >> 64) as u64;
    let bz_lo = bz as u64;
    let (mut rem_hi, mut rem_lo) = sub128(a0, a1, bz_hi, bz_lo);
    while (rem_hi as i64) < 0 {
        z = z.wrapping_sub(0x1_0000_0000);
        let b1 = b << 32;
        let (rh, rl) = add128(rem_hi, rem_lo, b0, b1);
        rem_hi = rh;
        rem_lo = rl;
    }
    let rem_hi2 = (rem_hi << 32) | (rem_lo >> 32);
    let lo_part: u64 = if (b0 << 32) <= rem_hi2 {
        0xFFFF_FFFF
    } else {
        rem_hi2 / b0
    };
    z | lo_part
}

/// Multiply 128-bit (a_hi, a_lo) by 64-bit b → 192-bit result (z0, z1, z2).
fn mul128by64to192(a_hi: u64, a_lo: u64, b: u64) -> (u64, u64, u64) {
    let lo_prod = (a_lo as u128) * (b as u128);
    let hi_prod = (a_hi as u128) * (b as u128);
    let z2 = lo_prod as u64;
    let mid = (lo_prod >> 64) + (hi_prod & 0xFFFF_FFFF_FFFF_FFFF);
    let z1 = mid as u64;
    let z0 = ((hi_prod >> 64) + (mid >> 64)) as u64;
    (z0, z1, z2)
}

fn add192(a0: u64, a1: u64, a2: u64, b0: u64, b1: u64, b2: u64) -> (u64, u64, u64) {
    let z2 = a2.wrapping_add(b2);
    let c1 = (z2 < a2) as u64;
    let z1_tmp = a1.wrapping_add(b1);
    let c0 = (z1_tmp < a1) as u64;
    let z1 = z1_tmp.wrapping_add(c1);
    let c0b = (z1 < c1) as u64;
    let z0 = a0.wrapping_add(b0).wrapping_add(c0).wrapping_add(c0b);
    (z0, z1, z2)
}

fn argument_reduction_kernel(sig0: u64, exp_diff: i32, z_sig0: &mut u64, z_sig1: &mut u64) -> u64 {
    let (a_sig1, a_sig0) = shift128_left(0, sig0, exp_diff as u32);
    let q = estimate_div128_to64(a_sig1, a_sig0, PI_HI);
    let (t0, t1, t2) = mul128by64to192(PI_HI, PI_LO, q);
    let (zs1, zs0) = sub128(a_sig1, a_sig0, t0, t1);
    let mut zs1 = zs1;
    let mut zs0 = zs0;
    let mut q = q;
    let mut t2 = t2;
    while (zs1 as i64) < 0 {
        q = q.wrapping_sub(1);
        let (a0, a1, a2) = add192(zs1, zs0, t2, 0, PI_HI, PI_LO);
        zs1 = a0;
        zs0 = a1;
        t2 = a2;
    }
    *z_sig1 = t2;
    *z_sig0 = zs0;
    let _ = zs1; // should be 0
    q
}

/// Reduce trig argument: returns quadrant (0..3) and updates z_sign, sig0, sig1.
fn reduce_trig_arg(exp_diff: i32, z_sign: &mut bool, sig0: &mut u64, sig1: &mut u64) -> i32 {
    let mut q: u64 = 0;

    if exp_diff < 0 {
        let (s0, s1) = shift128_right(*sig0, 0, 1);
        *sig0 = s0;
        *sig1 = s1;
        // exp_diff becomes 0
    } else if exp_diff > 0 {
        q = argument_reduction_kernel(*sig0, exp_diff, sig0, sig1);
    } else if PI_HI <= *sig0 {
        *sig0 -= PI_HI;
        q = 1;
    }

    let (term0, term1) = shift128_right(PI_HI, PI_LO, 1); // π/2
    if !lt128(*sig0, *sig1, term0, term1) {
        let lt = lt128(term0, term1, *sig0, *sig1);
        let eq = eq128(*sig0, *sig1, term0, term1);
        if (eq && (q & 1 != 0)) || lt {
            *z_sign = !*z_sign;
            q = q.wrapping_add(1);
        }
        if lt {
            let (s0, s1) = sub128(PI_HI, PI_LO, *sig0, *sig1);
            *sig0 = s0;
            *sig1 = s1;
        }
    }

    (q & 3) as i32
}

// ---------------------------------------------------------------------------
// FPTAN: tan(ST(0))  —  direct port of BOCHS fsincos.cc `ftan`
// ---------------------------------------------------------------------------

/// Compute tan(a).
/// Returns Some(result) or None if |a| >= 2^63 (out of range, caller sets C2).
pub fn ftan(a: F80) -> Option<F80> {
    let a_sig = a.mant;
    let a_exp = a.exp as i32;
    let a_sign = a.sign;

    if a_exp == 0x7FFF || a_exp == 0 {
        // ±0 → tan(0) = 0; infinity or NaN → return as-is (caller handles)
        return Some(a);
    }

    let z_exp = 16383i32; // FLOATX80_EXP_BIAS
    let exp_diff = a_exp - z_exp;

    // Out of range for argument reduction
    if exp_diff >= 63 {
        return None;
    }

    let mut z_sign = a_sign;
    let mut sig0 = a_sig;
    let mut sig1: u64 = 0;
    let q;
    let z_exp_out;

    if exp_diff < -1 {
        // Small argument: no reduction needed
        if exp_diff <= -68 {
            // tan(x) ≈ x for very small x
            return Some(a);
        }
        z_exp_out = a_exp;
        q = 0i32;
    } else {
        q = reduce_trig_arg(exp_diff, &mut z_sign, &mut sig0, &mut sig1);
        z_exp_out = z_exp; // after reduction, value is in [0, π/4]
    }

    // Pack reduced argument into F128
    // softfloat_normRoundPackToF128(0, z_exp - 0x10, sig0, sig1)
    let r = pack_from_floatx80_sig(z_exp_out, sig0, sig1);

    let sin_r = odd_poly(r, &SIN_ARR);
    let cos_r = even_poly(r, &COS_ARR);

    let result_f128 = if q & 1 != 0 {
        // cot: cos/sin, flip sign
        let r = f128_div(cos_r, sin_r);
        F128 {
            hi: r.hi ^ (1 << 63),
            lo: r.lo,
        } // negate
    } else {
        // tan: sin/cos
        f128_div(sin_r, cos_r)
    };

    let mut out = f128_to_f80(result_f128);
    if z_sign {
        out.sign = !out.sign;
    }
    Some(out)
}

/// Pack a floatx80 significand (sig0, sig1 as 128-bit extended) into F128.
/// Equivalent to softfloat_normRoundPackToF128(0, exp - 0x10, sig0, sig1).
fn pack_from_floatx80_sig(exp: i32, sig0: u64, sig1: u64) -> F128 {
    // sig0 is the 64-bit significand from argument reduction.  The leading 1
    // may be at any bit position (not necessarily bit 63 when the reduced
    // argument is smaller than the original).  Normalize first so that bit 63
    // is set, adjusting exp accordingly, then convert via f80_to_f128.
    if sig0 == 0 {
        return F128 { hi: 0, lo: 0 };
    }
    let lz = sig0.leading_zeros() as i32;
    let (norm_sig0, norm_sig1, norm_exp) = if lz == 0 {
        (sig0, sig1, exp)
    } else {
        // Shift the 128-bit (sig0, sig1) left by lz to place leading 1 at bit 63.
        let shift = lz as u32;
        let ns0 = (sig0 << shift) | (sig1 >> (64 - shift));
        let ns1 = sig1 << shift;
        (ns0, ns1, exp - lz)
    };

    let f = F80 {
        sign: false,
        exp: norm_exp as u16,
        mant: norm_sig0,
    };
    let mut f128 = f80_to_f128(f);
    // norm_sig1 holds the bits below norm_sig0.  After f80_to_f128 shifts
    // norm_sig0 right by 15 to form frac_lo, norm_sig1's top bits fill the
    // remaining low positions.
    f128.lo |= norm_sig1 >> 15;
    f128
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::f80::F80;

    fn f80_bytes(f: F80) -> [u8; 10] {
        let mut b = [0u8; 10];
        b[0..8].copy_from_slice(&f.mant.to_le_bytes());
        b[8..10].copy_from_slice(&f.exp.to_le_bytes());
        b
    }

    #[test]
    fn test_ftan_zero() {
        let zero = F80 {
            sign: false,
            exp: 0,
            mant: 0,
        };
        let result = ftan(zero).unwrap();
        let expected = [0u8; 10];
        let actual = f80_bytes(result);
        println!("ftan(0): {:02X?}", actual);
        assert_eq!(actual, expected, "ftan(0) should be 0");
    }

    #[test]
    fn test_ftan_pi4() {
        // pi/4 = F80 { exp: 0x3FFE, mant: 0xC90FDAA22168C235 }
        let pi4 = F80 {
            sign: false,
            exp: 0x3FFE,
            mant: 0xC90FDAA22168C235,
        };
        let result = ftan(pi4).unwrap();
        let expected: [u8; 10] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xFF, 0x3F];
        let actual = f80_bytes(result);
        println!("ftan(pi/4): {:02X?}", actual);
        assert_eq!(actual, expected, "ftan(pi/4) should be 1.0");
    }

    #[test]
    fn test_fpatan_zero_div_one() {
        // atan(0 / 1) = 0
        let one = F80::ONE;
        let zero = F80 {
            sign: false,
            exp: 0,
            mant: 0,
        };
        let result = fpatan(one, zero); // x=1, y=0
        let expected = [0u8; 10];
        let actual = f80_bytes(result);
        println!("fpatan(y=0, x=1): {:02X?}", actual);
        assert_eq!(actual, expected, "fpatan(0/1) should be 0");
    }

    #[test]
    fn test_fpatan_one_div_one() {
        // atan(1 / 1) = pi/4
        let one = F80::ONE;
        let result = fpatan(one, one); // x=1, y=1
        let expected: [u8; 10] = [0x35, 0xC2, 0x68, 0x21, 0xA2, 0xDA, 0x0F, 0xC9, 0xFE, 0x3F];
        let actual = f80_bytes(result);
        println!("fpatan(y=1, x=1): {:02X?}", actual);
        assert_eq!(actual, expected, "fpatan(1/1) should be pi/4");
    }

    #[test]
    fn test_fpatan_one_div_zero() {
        // atan(1 / 0) = pi/2
        let one = F80::ONE;
        let zero = F80 {
            sign: false,
            exp: 0,
            mant: 0,
        };
        let result = fpatan(zero, one); // x=0, y=1
        let expected: [u8; 10] = [0x35, 0xC2, 0x68, 0x21, 0xA2, 0xDA, 0x0F, 0xC9, 0xFF, 0x3F];
        let actual = f80_bytes(result);
        println!("fpatan(y=1, x=0): {:02X?}", actual);
        assert_eq!(actual, expected, "fpatan(1/0) should be pi/2");
    }

    fn f128_to_hex(f: F128) -> String {
        format!("{:016X}_{:016X}", f.hi, f.lo)
    }

    /// Replicate the checkit trig test: compute atan(1/tan(pi/5)) and
    /// compare to the known-correct double 3*pi/10.
    #[test]
    fn test_checkit_trig_trace() {
        let pi = F80::PI;
        let one = F80::ONE;
        let two = one.add(one);
        let four = two.mul(two);
        let five = four.add(one);
        let pi_over_5 = pi.div(five);
        let x = ftan(pi_over_5).unwrap(); // tan(pi/5)
        let y = one;

        let x128 = f80_to_f128(F80 { sign: false, ..x });
        let y128 = f80_to_f128(F80 { sign: false, ..y });
        println!("x128 = {}", f128_to_hex(x128));
        println!("y128 = {}", f128_to_hex(y128));

        // y > x so swap=true, arg = x/y = x (y=1)
        let arg0 = f128_div(x128, y128);
        println!("arg0 (=x/1) = {}", f128_to_hex(arg0));

        // Arg reduction: (arg*√3 - 1) / (arg + √3)
        let t1a = f128_mul(arg0, F128_SQRT3);
        println!("arg*sqrt3 = {}", f128_to_hex(t1a));
        let t2 = f128_add(arg0, F128_SQRT3);
        println!("arg+sqrt3 = {}", f128_to_hex(t2));
        let t1b = f128_sub(t1a, F128_ONE);
        println!("arg*sqrt3-1 = {}", f128_to_hex(t1b));
        let arg_reduced = f128_div(t1b, t2);
        println!("arg_reduced = {}", f128_to_hex(arg_reduced));

        let poly_result = odd_poly(arg_reduced, &ATAN_ARR);
        println!("poly = {}", f128_to_hex(poly_result));

        let plus_pi6 = f128_add(poly_result, F128_PI6);
        println!("poly+pi6 = {}", f128_to_hex(plus_pi6));

        let final_result = f128_sub(F128_PI2, plus_pi6);
        println!("pi2 - result = {}", f128_to_hex(final_result));

        let out = f128_to_f80(final_result);
        println!("f80 result: {:02X?}", f80_bytes(out));
        println!("f64 result: 0x{:016X}", out.to_f64().to_bits());
        println!("expected:   0x3FEE28C731EB6950");
    }

    /// Test fpatan with exact tan(pi/5) to see if error is in ftan or fpatan.
    #[test]
    fn test_checkit_isolate() {
        // Use f64-rounded tan(pi/5) as input to see if fpatan alone produces the right answer.
        let tan_pi5_f64: f64 = (std::f64::consts::PI / 5.0).tan();
        let tan_pi5_f80 = F80::from_f64(tan_pi5_f64);
        println!("tan(pi/5) f64 input: 0x{:016X}", tan_pi5_f64.to_bits());
        println!("tan(pi/5) f80: {:02X?}", f80_bytes(tan_pi5_f80));
        let one = F80::ONE;
        let result = fpatan(tan_pi5_f80, one);
        println!("fpatan result f64: 0x{:016X}", result.to_f64().to_bits());
        println!("expected:          0x3FEE28C731EB6950");
    }

    #[test]
    fn test_ftan_trace() {
        let pi = F80::PI;
        let one = F80::ONE;
        let two = one.add(one);
        let four = two.mul(two);
        let five = four.add(one);
        let pi_over_5 = pi.div(five);

        // Reproduce ftan internals step by step
        let a = pi_over_5;
        let a_exp = a.exp as i32;
        let z_exp = 16383i32;
        let exp_diff = a_exp - z_exp;
        let mut z_sign = a.sign;
        let mut sig0 = a.mant;
        let mut sig1: u64 = 0;
        let q = reduce_trig_arg(exp_diff, &mut z_sign, &mut sig0, &mut sig1);
        println!(
            "q={}, z_sign={}, sig0={:016X}, sig1={:016X}",
            q, z_sign, sig0, sig1
        );

        let r = pack_from_floatx80_sig(z_exp, sig0, sig1);
        println!("r = {}", f128_to_hex(r));

        let x2 = f128_mul(r, r);
        println!("x2 (r^2) = {}", f128_to_hex(x2));

        // Trace sin polynomial: odd_poly(r, SIN_ARR) = r * eval_poly(x2, SIN_ARR)
        let n = SIN_ARR.len();
        let mut rs = SIN_ARR[n - 1];
        println!("sin eval start: rs = {}", f128_to_hex(rs));
        for i in (0..n - 1).rev() {
            rs = f128_muladd(rs, x2, SIN_ARR[i]);
            println!(
                "sin eval step {} (coeff {}): rs = {}",
                n - 1 - i,
                i,
                f128_to_hex(rs)
            );
        }
        let sin_r = f128_mul(r, rs);
        println!("sin_r = {}", f128_to_hex(sin_r));

        // Trace cos polynomial: even_poly(r, COS_ARR) = eval_poly(x2, COS_ARR)
        let n = COS_ARR.len();
        let mut rc = COS_ARR[n - 1];
        println!("cos eval start: rc = {}", f128_to_hex(rc));
        for i in (0..n - 1).rev() {
            rc = f128_muladd(rc, x2, COS_ARR[i]);
            println!(
                "cos eval step {} (coeff {}): rc = {}",
                n - 1 - i,
                i,
                f128_to_hex(rc)
            );
        }
        let cos_r = rc;
        println!("cos_r = {}", f128_to_hex(cos_r));

        let result_f128 = f128_div(sin_r, cos_r);
        println!("sin/cos = {}", f128_to_hex(result_f128));

        let out = f128_to_f80(result_f128);
        println!("ftan result: {:02X?}", f80_bytes(out));
    }

    #[test]
    fn test_checkit_trig() {
        // Compute pi/5 the same way checkit does:
        //   fldpi; fld1; fadd; fmul; fld1; faddp; fdivp
        let pi = F80::PI;
        let one = F80::ONE;
        let two = one.add(one); // 2.0
        let four = two.mul(two); // 4.0
        let five = four.add(one); // 5.0
        let pi_over_5 = pi.div(five);

        println!("pi/5 bytes: {:02X?}", f80_bytes(pi_over_5));

        // fptan(pi/5) → ST(0)=1.0, ST(1)=tan(pi/5)
        let tan_pi5 = ftan(pi_over_5).unwrap();
        println!("ftan(pi/5) bytes: {:02X?}", f80_bytes(tan_pi5));

        // fpatan(x=tan(pi/5), y=1.0) = atan(1/tan(pi/5)) = 3*pi/10
        let result = fpatan(tan_pi5, one);
        println!("fpatan(y=1, x=tan(pi/5)) bytes: {:02X?}", f80_bytes(result));

        // Convert to double (what fstp qword produces)
        let result_f64 = result.to_f64();
        let result_bits = result_f64.to_bits();
        println!("as f64: {:.20} (0x{:016X})", result_f64, result_bits);

        // Expected: 3*pi/10 as f64
        let expected_f64 = 3.0f64 * std::f64::consts::PI / 10.0;
        let expected_bits = expected_f64.to_bits();
        println!("expected: {:.20} (0x{:016X})", expected_f64, expected_bits);

        assert_eq!(
            result_bits, expected_bits,
            "fpatan(1/tan(pi/5)) as double should equal 3*pi/10"
        );
    }
}
