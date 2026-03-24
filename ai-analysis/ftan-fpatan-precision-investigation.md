# FTAN/FPATAN Precision Investigation

## Problem

`test_checkit_trig` fails with a 1 ULP error:

```
fpatan(tan(pi/5), 1.0) → 0x3FEE28C731EB6951  (our result)
                          0x3FEE28C731EB6950  (expected, verified 8087 hardware output)
```

The test replicates what the `checkit` benchmark does:
1. Compute pi/5 using F80 arithmetic (fldpi → div by 5.0)
2. Call `ftan(pi/5)` → tan(pi/5) as F80
3. Call `fpatan(tan(pi/5), 1.0)` → should equal 3π/10
4. Convert to f64 via `fstp qword`

## Key Isolation Result

`test_checkit_isolate` confirmed: **fpatan is correct**. When fed the Rust f64-computed `tan(pi/5)` (converted to F80), fpatan returns the exact expected value `0x3FEE28C731EB6950`. The error is in `ftan`.

Our `ftan(pi/5)` produces F80 `{exp=0x3FFE, mant=0xB9FECB0EEFAA1459}`, which is slightly off from what hardware would produce. This slightly-wrong input causes fpatan to round 1 ULP high.

## Architecture: Bochs F128 Precision Reduction

Bochs (the reference implementation, `tmp/Bochs`) uses SoftFloat-3e for f128 arithmetic. Critically, Bochs's `softfloat_roundPackToF128` applies an **intentional precision reduction**:

```c
// s_roundPackToF128.c
sigExtra = 0; // artificially reduce precision to match hardware x86 which uses only 67-bit
sig0 &= UINT64_C(0xFFFFFFFF00000000); // do 80 bits for now
```

This zeroes the lower 32 bits of `sig0` (the low 64-bit word of the f128 significand).

## Which Operations Apply Truncation in Bochs

The truncation happens via `softfloat_roundPackToF128`. Bochs calls this from:

| Operation | Path | Truncates? |
|-----------|------|-----------|
| `f128_mul` | → `softfloat_roundPackToF128` | **YES** |
| `f128_add` | → `softfloat_roundPackToF128` | **YES** |
| `f128_div` | → `softfloat_roundPackToF128` | **YES** |
| `f128_mulAdd` | → `softfloat_roundPackToF128` | **YES** |
| `f128_sub` | → `softfloat_normRoundPackToF128` | **NO** (fast path) |
| Initial f80→f128 (normRoundPackToF128) | fast path | **NO** |

The fast path in `softfloat_normRoundPackToF128` for normal exponents (`exp < 0x7FFD`):

```c
if ((uint32_t) exp < 0x7FFD) {
    z.v64 = packToF128UI64(sign, sig64 | sig0 ? exp : 0, sig64);
    z.v0  = sig0;
    return z;   // <-- returns WITHOUT calling softfloat_roundPackToF128
}
```

This means subtraction results are stored with **full precision** in Bochs (the lower 32 bits of sig0 can be non-zero after a subtraction), while mul/add/div results have those bits zeroed.

## Bochs Polynomial Evaluation

Bochs uses a fused multiply-add (`f128_mulAdd`) for polynomial evaluation in `poly.cc`:

```c
float128_t EvalPoly(float128_t x, const float128_t *arr, int n, ...) {
    float128_t r = arr[--n];
    do {
        r = f128_mulAdd(r, x, arr[--n], 0, &status);  // fused, single rounding
    } while (n > 0);
    return r;
}
```

Every iteration applies truncation via `softfloat_roundPackToF128`.

## Pseudocode from exec logs

From `oxide86.trig-fail.log` lines 3537488–3574533 (the 8087 checker program, segment `269D`).

### FPU test function at 269D:0171

Called from `2580:06A8` with `test_number = 1`.

```
void fpu_test(int test_number) {

    // --- TEST 1: f2xm1 + fyl2x precision test ---
    // Log line 3563715
    fninit();                           // Reset FPU

    // Compute: (2^(1/3) - 1) * log2(pi)
    //
    // fld1          → ST(0) = 1.0
    // fadd st0,st0  → ST(0) = 2.0
    // fld1          → ST(0) = 1.0, ST(1) = 2.0
    // fadd st1,st0  → ST(0) = 1.0, ST(1) = 3.0
    // fdivrp st1,st0→ ST(0) = 1.0 / 3.0 = 0.33333...
    // f2xm1         → ST(0) = 2^(1/3) - 1
    // fldpi         → ST(0) = pi, ST(1) = 2^(1/3) - 1
    // fyl2x         → ST(0) = (2^(1/3) - 1) * log2(pi)

    result = f2xm1_yl2x_computation();
    fstp qword [0x6E68];               // store result

    delay_loop(0x0FFF);                 // busy wait for 8087 to finish

    if (test_number != 3) {
        // Compare 8-byte result at [0x6E68] vs expected at [0x6E58]
        if (memcmp([0x6E68], [0x6E58], 8) != 0)
            goto fail;
    }

    // --- TEST 2: fptan + fpatan round-trip precision test ---
    // Log line 3567852
    //
    // Compute: fpatan(fptan(pi/5)) — should recover pi/5 exactly
    //
    // fldpi          → ST(0) = pi
    // fld1           → ST(0) = 1.0, ST(1) = pi
    // fadd st0,st0   → ST(0) = 2.0, ST(1) = pi
    // fmul st0,st0   → ST(0) = 4.0, ST(1) = pi
    // fld1           → ST(0) = 1.0, ST(1) = 4.0, ST(2) = pi
    // faddp st1,st0  → ST(0) = 5.0, ST(1) = pi
    // fdivp st1,st0  → ST(0) = pi / 5.0
    // fptan          → ST(0) = 1.0 (pushed), ST(1) = tan(pi/5) (replaced)
    // fxch st1       → ST(0) = tan(pi/5), ST(1) = 1.0
    // fpatan         → ST(0) = atan(ST(1)/ST(0)) = atan(1.0/tan(pi/5)) = atan(cot(pi/5))

    result = fptan_fpatan_roundtrip();
    fstp qword [0x6E68];               // store result

    delay_loop(0x0FFF);

    if (test_number != 3) {
        // Compare 8-byte result at [0x6E68] vs expected at [0x6E60]
        if (memcmp([0x6E68], [0x6E60], 8) != 0)
            goto fail;
    }

fail:
    return 1;
}
```

### Helper: memcmp at 269D:0248

```
// rep cmpsb comparing 8 bytes: computed [0x6E68] vs expected [SI]
// Sets flags for jne to branch on mismatch
void compare_result(byte *expected_ptr) {
    es = ds;
    di = 0x6E68;       // computed result
    cx = 8;
    rep cmpsb;          // byte-by-byte comparison
}
```

### Helper: delay_loop at 269D:025A

```
void delay_loop() {
    cx = 0x0FFF;        // 4095 iterations
    while (cx--) {}     // busy wait for 8087 coprocessor
}
```

### Summary of the two FPU tests:

Test	Computation	What it validates
Test 1 (line 3563715)	(2^(1/3) - 1) * log2(pi) via f2xm1 + fyl2x	Precision of f2xm1 and fyl2x transcendental instructions
Test 2 (line 3567852)	fpatan(fptan(pi/5)) round-trip — should recover pi/5 exactly	Precision of fptan and fpatan as inverse operations

Each test:

1. Performs the FPU computation and stores the 8-byte IEEE double result to [0x6E68]
1. Runs a busy-wait delay loop (0x0FFF iterations) — likely to ensure the 8087 coprocessor has finished
1. Compares the 8-byte result byte-for-byte against a pre-stored expected value ([0x6E58] for test 1, [0x6E60] for test 2) using rep cmpsb
1. If the bytes don't match exactly, jumps to the fail path

The test_number parameter (value 1 here) controls whether comparisons happen — when test_number == 3 the comparisons are skipped (presumably a "just compute, don't verify" mode). Since test_number == 1 here, both comparisons execute. The function returns ax = 1 regardless — the branching at jne 0x01DE skips test 2 on test 1 failure but always returns 1 (the caller at 2580:06A8 likely uses this differently, or the "Passed"/"Failed" logic is in the text the caller formats).

### Key observations

- **Test 2 is the fptan/fpatan round-trip** that this investigation is about
- The test does **exact byte comparison** (not epsilon) — the emulated FPU must produce bit-identical IEEE doubles
- `test_number == 3` mode skips comparisons (compute-only / benchmark mode)
- Expected values are pre-stored at `[0x6E58]` (test 1) and `[0x6E60]` (test 2)
- Both tests always execute sequentially; test 1 failure skips test 2

## Our Implementation's Differences

### 1. `pack_f128` (our truncation point)

We added Bochs-style truncation to `pack_f128`:

```rust
fn pack_f128(sign: bool, exp: i32, sig: u128) -> F128 {
    let frac_lo = (frac as u64) & 0xFFFF_FFFF_0000_0000;  // zero lower 32 bits
    ...
}
```

This is called from `pack_rounded`, which is called from mul, add, div. **Also called directly from `f128_sub_mags`**, so our subtraction DOES truncate — which is **more lossy** than Bochs where subtraction does NOT truncate.

### 2. `pack_from_floatx80_sig` (initial f80→f128)

```rust
fn pack_from_floatx80_sig(exp: i32, sig0: u64, sig1: u64) -> F128 {
    // ... normalize ...
    let mut f128 = f80_to_f128(f);
    f128.lo |= norm_sig1 >> 15;  // can set bits anywhere in lo
    f128
}
```

This does NOT apply truncation. The lower bits can be non-zero. This matches Bochs's behavior (normRoundPackToF128 fast path doesn't truncate).

### 3. `f128_muladd` subtraction case

When a*b and c have opposite signs, our `f128_muladd` separates into:
1. Round the product → truncates
2. Call `f128_add(prod, c)` → which calls `f128_sub_mags` → truncates again

Bochs's `f128_mulAdd` handles this as a single operation with one rounding at the end.

## Summary of Discrepancies

1. **Subtraction truncation mismatch**: Bochs subtraction does NOT truncate lower 32 bits of sig0 (fast path in normRoundPackToF128). Our subtraction DOES truncate. This means our subtraction introduces more precision loss.

2. **muladd subtraction path**: Our `f128_muladd` with opposite signs rounds the product separately before subtracting. Bochs does a true fused operation.

## What Needs to Change

The core problem: `ftan(pi/5)` produces a slightly wrong result that causes `fpatan` to be off by 1 ULP.

The mismatch in subtraction truncation (our subtraction truncates; Bochs's does not) is a structural difference that affects all intermediate computations in the polynomial evaluation.

**Fix approach**: Remove the truncation from `f128_sub_mags` (don't call `pack_f128`, call a version without the lower-32-bit zeroing), to match Bochs's behavior where subtraction results preserve full sig0 precision.

However, this is not simple because:
- If we stop truncating in subtraction, we might break other tests
- The muladd subtraction path has a different structure than Bochs's fused operation
- The exact sequence of truncations in ftan's polynomial evaluation (sin/cos computation) produces the slightly wrong tan(pi/5)

## Reference Files

- `tmp/Bochs/bochs/cpu/fpu/fsincos.cc` — ftan implementation
- `tmp/Bochs/bochs/cpu/fpu/poly.cc` — EvalPoly, OddPoly, EvenPoly
- `tmp/Bochs/bochs/cpu/softfloat3e/s_roundPackToF128.c` — truncation site
- `tmp/Bochs/bochs/cpu/softfloat3e/s_normRoundPackToF128.c` — fast path (no truncation)
- `tmp/Bochs/bochs/cpu/softfloat3e/s_subMagsF128.c` — subtraction uses normRoundPack
- `tmp/Bochs/bochs/cpu/softfloat3e/f128_mul.c` — uses roundPackToF128 (truncates)
- `tmp/Bochs/bochs/cpu/softfloat3e/f128_div.c` — uses roundPackToF128 (truncates)
- `tmp/Bochs/bochs/cpu/softfloat3e/f128_mulAdd.c` — fused op, uses roundPackToF128
- `core/src/cpu/f80_trig.rs` — our implementation
