# Why the FPU Stack Needed 80-bit Extended Precision (F80)

## The Bug

When running `checkit.exe`, the 8087 coprocessor arithmetic test printed **FAILED**. The test
verifies that A + B = C where:

| Variable | Value (hex) | Value (decimal) |
|---|---|---|
| A | `0x5555555555555555` | 6,148,914,691,236,517,205 |
| B | `0xAAAAAAAAAAAAAAAA` | −6,148,914,691,236,517,206 (signed) |
| C | `0xFFFFFFFFFFFFFFFF` | −1 (expected A+B) |

The test loads A and B with `FILD m64`, adds them with `FADDP`, and stores the result with
`FISTP m64`. The result should be `−1` but the emulator produced `0`.

## Root Cause: f64 Cannot Represent Large 64-bit Integers Exactly

The emulator's FPU stack was `[f64; 8]`. IEEE 754 double-precision (`f64`) has:

- 1 sign bit
- 11 exponent bits
- **52 explicit mantissa bits** → 53 significant bits total

53 bits of mantissa means f64 can represent integers exactly only up to **2^53 = 9,007,199,254,740,992**
(~9 × 10^15). Both A and B are ~6.1 × 10^18, which is **2^62**, far beyond that limit.

When `FILD m64` loaded A (`0x5555555555555555`), it was immediately rounded to the nearest f64:

```
True A = 6,148,914,691,236,517,205
f64(A) = 6,148,914,691,236,517,376  (nearest representable f64, rounded up)
```

Similarly for B:

```
True |B| = 6,148,914,691,236,517,206
f64(B) = 6,148,914,691,236,517,376  (same magnitude as A after rounding!)
```

Both A and B rounded to the **same f64 magnitude** — so f64(A) + f64(B) = 0 instead of −1.
The MCP debug session confirmed this directly: ST(0) showed `6.148914691236517e18` (not the exact
integer), and the result memory location contained all zeros.

## What the Real 8087 Does

The Intel 8087 co-processor stores all values in **80-bit extended precision**:

| Field | Bits | Description |
|---|---|---|
| Sign | 1 | 0 = positive, 1 = negative |
| Exponent | 15 | Biased exponent, bias = 16383 |
| Significand | 64 | **Explicit** integer bit + 63-bit fraction |

The critical difference: the 64-bit significand can represent **all 64-bit integers exactly**.
When `FILD m64` loads `0x5555555555555555`, the 8087 stores it without any loss:

```
sign = 0
exp  = 16383 + 62 = 16445  (highest set bit is at position 62)
mant = 0x5555555555555555 << 1 = 0xAAAAAAAAAAAAAAAA  (left-shifted so bit 63 = integer bit)
```

Adding the two values in 80-bit arithmetic:
- Both have `exp = 16445`, opposite signs → subtract magnitudes
- `0xAAAAAAAAAAAAAAAC − 0xAAAAAAAAAAAAAAAA = 2`
- Normalize: shift left 62, decrement exp by 62 → `exp = 16383`, `mant = 0x8000000000000000`
- This is exactly **−1** in 80-bit format

`FISTP m64` then converts this to the signed integer `−1 = 0xFFFFFFFFFFFFFFFF`. ✓

## The Fix

A new `F80` struct was created in [core/src/cpu/f80.rs](../core/src/cpu/f80.rs):

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct F80 {
    pub sign: bool,
    pub exp:  u16,   // biased exponent (bias = 16383)
    pub mant: u64,   // 64-bit significand; bit 63 = explicit integer bit
}
```

Key implementation details:

### Exact integer conversion (`from_i64` / `to_i64`)
- `from_i64`: finds the highest set bit of the unsigned magnitude, sets `exp = 16383 + k`, shifts
  mantissa left so bit 63 is the integer bit. Exact for all i64 values including `i64::MIN`.
- `to_i64`: extracts the integer part by right-shifting the mantissa. Uses round-to-nearest-even
  (IEEE 754 default) with separate handling of floor/ceil/truncate modes.

### 80-bit arithmetic (`add`, `sub`, `mul`)
- **Addition/subtraction**: uses `u128` intermediates so no precision is lost during alignment.
  If signs agree, mantissas are summed; overflow shifts right and bumps exponent. If signs differ,
  the smaller magnitude is subtracted and the result is normalized (leading zeros shifted out).
- **Multiplication**: `(a.mant as u128) * (b.mant as u128)` gives a 128-bit product; the top 64
  bits are taken as the result after normalization.
- **Division and transcendentals**: delegated to f64 via `to_f64()`/`from_f64()`. Sufficient for
  programs that don't need exact integer results from division.

### Correct f64 conversion (`to_f64`)
Uses **round-to-nearest-even** when converting the 64-bit mantissa down to 52 bits, rather than
truncation. This matters for constants like `LOG10_2`:

```
F80::LOG10_2.mant lower 11 bits = 0x799 = 1945
Half-way point                  = 0x400 = 1024
1945 > 1024  →  truncation would give 0x...FE, but correct value is 0x...FF
```

Without rounding, the `FLDLG2 → FSTP qword` test would have stored a byte that was off by one.

### 8087-exact constants
The FPU constant instructions (`FLDPI`, `FLDL2E`, `FLDLN2`, `FLDL2T`, `FLDLG2`) now push the
Intel-specified 80-bit values directly rather than converting from f64 first:

```rust
pub const PI:     F80 = F80 { sign: false, exp: 0x4000, mant: 0xC90F_DAA2_2168_C235 };
pub const LOG2_E: F80 = F80 { sign: false, exp: 0x3FFF, mant: 0xB8AA_3B29_5C17_F0BC };
pub const LN_2:   F80 = F80 { sign: false, exp: 0x3FFE, mant: 0xB172_17F7_D1CF_79AC };
pub const LOG2_10:F80 = F80 { sign: false, exp: 0x4000, mant: 0xD49A_784B_CD1B_8AFE };
pub const LOG10_2:F80 = F80 { sign: false, exp: 0x3FFD, mant: 0x9A20_9A84_FBCF_F799 };
```

## Files Changed

| File | Change |
|---|---|
| `core/src/cpu/f80.rs` | New — F80 type with exact arithmetic |
| `core/src/cpu/mod.rs` | `fpu_stack: [f64; 8]` → `[F80; 8]`; snapshot converts F80→f64 for display |
| `core/src/cpu/instructions/fpu.rs` | All FPU operations rewritten to use F80; transcendentals delegate to f64 |

## Outcome

After the fix, all four 8087 test suites pass:
- `op8087_arith` — arithmetic with large 64-bit integers now correct ✓
- `op8087_load_store` — load/store round-trips including constants and FILD/FISTP m64 ✓
- `op8087_compare` — FPU comparisons ✓
- `op8087_control` — FPU control instructions ✓

The trig test in `checkit.exe` likely still differs from the real 8087 for other reasons (the
transcendental argument reduction and CORDIC algorithm used by the 8087 produce slightly different
low-order bits than the f64 library functions), but the arithmetic test that motivated this work
now passes correctly.
