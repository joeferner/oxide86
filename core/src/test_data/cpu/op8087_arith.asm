; op8087_arith.asm - 8087 FPU arithmetic instruction tests
;
; Tests:
;   FADD/FSUB/FSUBR/FMUL/FDIV/FDIVR (pop forms via DE prefix)
;   FADD/FSUB/FSUBR/FMUL/FDIV/FDIVR m32 (memory operand, D8 prefix)
;   FADD/FSUB/FMUL/FDIV ST,ST(i) (register non-pop, D8 prefix)
;   FSQRT, FABS, FCHS, FRNDINT
;   FPTAN, FPATAN, F2XM1, FYL2X
;   FYL2XP1 - ST(1)*log2(ST(0)+1), pops
;   FPREM   - partial remainder: ST(0) mod ST(1)
;   FSCALE  - ST(0) *= 2^TRUNC(ST(1))
;   FXTRACT - split ST(0) into significand and exponent
;
; All tests compare the result to a pre-computed expected value stored
; in the data section as an IEEE 754 double (64-bit).
;
; Exit codes:
;   0x00 = all tests passed
;   0x01 = one or more tests failed

[CPU 8086]
[ORG 0x100]

section .text
start:
    mov word [pass_count], 0
    mov word [fail_count], 0

    call test_fadd
    call test_fsub
    call test_fmul
    call test_fdiv
    call test_fsqrt
    call test_fabs
    call test_fchs
    call test_frndint
    call test_fptan
    call test_fpatan
    call test_f2xm1
    call test_fyl2x
    call test_arith_mem
    call test_arith_reg
    call test_fyl2xp1
    call test_fprem
    call test_fscale
    call test_fxtract

    cmp word [fail_count], 0
    jne .fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

.fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

;=============================================================================
; Test: FADD — ST(0) = 1.0 + 2.0 = 3.0
;=============================================================================
test_fadd:
    fninit
    fld qword [val_1f64]        ; ST(0) = 1.0
    fld qword [val_2f64]        ; ST(0) = 2.0, ST(1) = 1.0
    fadd                        ; ST(0) = 3.0, pops one
    fstp qword [scratch64]
    mov si, val_3f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FSUB — FSUBP ST(1),ST(0): ST(1) = ST(1) - ST(0), pop → 3.0 - 1.0 = 2.0
;=============================================================================
test_fsub:
    fninit
    fld qword [val_3f64]        ; ST(0) = 3.0  (will be ST(1) after next push)
    fld qword [val_1f64]        ; ST(0) = 1.0, ST(1) = 3.0
    fsub                        ; FSUBP: ST(1) = 3.0 - 1.0 = 2.0, pop → ST(0) = 2.0
    fstp qword [scratch64]
    mov si, val_2f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FMUL — ST(0) = 2.0 * 3.0 = 6.0
;=============================================================================
test_fmul:
    fninit
    fld qword [val_2f64]        ; ST(0) = 2.0
    fld qword [val_3f64]        ; ST(0) = 3.0, ST(1) = 2.0
    fmul                        ; ST(0) = 6.0, pops one
    fstp qword [scratch64]
    mov si, val_6f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FDIV — FDIVP ST(1),ST(0): ST(1) = ST(1) / ST(0), pop → 6.0 / 2.0 = 3.0
;=============================================================================
test_fdiv:
    fninit
    fld qword [val_6f64]        ; ST(0) = 6.0  (will be ST(1) after next push)
    fld qword [val_2f64]        ; ST(0) = 2.0, ST(1) = 6.0
    fdiv                        ; FDIVP: ST(1) = 6.0 / 2.0 = 3.0, pop → ST(0) = 3.0
    fstp qword [scratch64]
    mov si, val_3f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FSQRT — sqrt(4.0) = 2.0
;=============================================================================
test_fsqrt:
    fninit
    fld qword [val_4f64]        ; ST(0) = 4.0
    fsqrt                       ; ST(0) = 2.0
    fstp qword [scratch64]
    mov si, val_2f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FABS — abs(-2.0) = 2.0
;=============================================================================
test_fabs:
    fninit
    fld qword [val_neg2f64]     ; ST(0) = -2.0
    fabs                        ; ST(0) = 2.0
    fstp qword [scratch64]
    mov si, val_2f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FCHS — -(2.0) = -2.0
;=============================================================================
test_fchs:
    fninit
    fld qword [val_2f64]        ; ST(0) = 2.0
    fchs                        ; ST(0) = -2.0
    fstp qword [scratch64]
    mov si, val_neg2f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FRNDINT — round(2.7) = 3.0
;=============================================================================
test_frndint:
    fninit
    fld qword [val_2_7f64]      ; ST(0) = 2.7
    frndint                     ; ST(0) = 3.0  (round to nearest)
    fstp qword [scratch64]
    mov si, val_3f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FPTAN — tan(0.0) = 0.0, pushes 1.0
; After FPTAN: ST(0)=1.0, ST(1)=0.0
; Pop 1.0 (discard), then compare ST(0)=0.0
;=============================================================================
test_fptan:
    fninit
    fld qword [val_0f64]        ; ST(0) = 0.0
    fptan                       ; ST(0) = 1.0, ST(1) = tan(0) = 0.0
    fstp qword [scratch64]      ; pop and discard 1.0
    fstp qword [scratch64]      ; pop tan result = 0.0
    mov si, val_0f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FPATAN — atan(1.0 / 1.0) = pi/4
; Load ST(0) = 1.0 (y), ST(1) = 1.0 (x) then FPATAN → ST(0) = atan(y/x) = pi/4
; Note: FPATAN computes atan(ST(1)/ST(0)) and pops, so:
;   push x=1.0 first, then y=1.0, FPATAN → atan(1.0/1.0) = pi/4
;=============================================================================
test_fpatan:
    fninit
    fld qword [val_1f64]        ; ST(0) = 1.0  (x, will be ST(1) after next push)
    fld qword [val_1f64]        ; ST(0) = 1.0  (y), ST(1) = 1.0 (x)
    fpatan                      ; ST(0) = atan(ST(1)/ST(0)) = atan(1.0/1.0) = pi/4, pops
    fstp qword [scratch64]
    mov si, val_pi4f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: F2XM1 — 2^0.0 - 1 = 0.0
;=============================================================================
test_f2xm1:
    fninit
    fld qword [val_0f64]        ; ST(0) = 0.0
    f2xm1                       ; ST(0) = 2^0.0 - 1 = 0.0
    fstp qword [scratch64]
    mov si, val_0f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FYL2X — 2.0 * log2(4.0) = 4.0
; Push x=4.0, then y=2.0. FYL2X: ST(0) = ST(1)*log2(ST(0)), pops
;=============================================================================
test_fyl2x:
    fninit
    fld qword [val_4f64]        ; ST(0) = 4.0
    fld qword [val_2f64]        ; ST(0) = 2.0 (y), ST(1) = 4.0 (x)
    fyl2x                       ; ST(0) = 2.0 * log2(4.0) = 4.0, pops
    fstp qword [scratch64]
    mov si, val_4f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: arithmetic memory operand variants (D8 prefix, m32)
; Each loads a value into ST(0) then applies a m32 operation.
;   FADD  m32: ST(0) = ST(0) + m32   (D8 /0)
;   FSUB  m32: ST(0) = ST(0) - m32   (D8 /4)
;   FSUBR m32: ST(0) = m32 - ST(0)   (D8 /5)
;   FMUL  m32: ST(0) = ST(0) * m32   (D8 /1)
;   FDIV  m32: ST(0) = ST(0) / m32   (D8 /6)
;   FDIVR m32: ST(0) = m32 / ST(0)   (D8 /7)
;=============================================================================
test_arith_mem:
    ; FADD m32: 2.0 + 1.0 = 3.0
    fninit
    fld dword [val_2f32]
    fadd dword [val_1f32]
    fstp qword [scratch64]
    mov si, val_3f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes

    ; FSUB m32: 3.0 - 1.0 = 2.0
    fninit
    fld dword [val_3f32]
    fsub dword [val_1f32]
    fstp qword [scratch64]
    mov si, val_2f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes

    ; FSUBR m32: 3.0 - 1.0 = 2.0  (m32=3.0, ST(0)=1.0, result = m32 - ST(0))
    fninit
    fld dword [val_1f32]
    fsubr dword [val_3f32]
    fstp qword [scratch64]
    mov si, val_2f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes

    ; FMUL m32: 2.0 * 3.0 = 6.0
    fninit
    fld dword [val_2f32]
    fmul dword [val_3f32]
    fstp qword [scratch64]
    mov si, val_6f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes

    ; FDIV m32: 6.0 / 2.0 = 3.0
    fninit
    fld dword [val_6f32]
    fdiv dword [val_2f32]
    fstp qword [scratch64]
    mov si, val_3f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes

    ; FDIVR m32: 6.0 / 2.0 = 3.0  (m32=6.0, ST(0)=2.0, result = m32 / ST(0))
    fninit
    fld dword [val_2f32]
    fdivr dword [val_6f32]
    fstp qword [scratch64]
    mov si, val_3f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: arithmetic register non-pop variants (D8 prefix: ST,ST(i))
; These modify ST(0) in place without popping.
;   FADD ST0,ST1: ST(0) += ST(1)  → 2.0 + 1.0 = 3.0
;   FSUB ST0,ST1: ST(0) -= ST(1)  → 3.0 - 1.0 = 2.0
;   FMUL ST0,ST1: ST(0) *= ST(1)  → 3.0 * 2.0 = 6.0
;   FDIV ST0,ST1: ST(0) /= ST(1)  → 6.0 / 2.0 = 3.0
; Also tests one DC variant: FADD ST1,ST0 → ST(1) += ST(0)
;=============================================================================
test_arith_reg:
    ; FADD ST0, ST1: 2.0 + 1.0 = 3.0
    ; Result is in ST(0); compare before popping the unchanged ST(1).
    fninit
    fld qword [val_1f64]        ; ST(0)=1.0 → will be ST(1)
    fld qword [val_2f64]        ; ST(0)=2.0, ST(1)=1.0
    fadd st0, st1               ; D8 C1: ST(0) = 2.0 + 1.0 = 3.0, ST(1) unchanged
    fstp qword [scratch64]      ; pop result ST(0)=3.0
    mov si, val_3f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    fstp qword [scratch64]      ; discard ST(1)=1.0

    ; FSUB ST0, ST1: 3.0 - 1.0 = 2.0
    fninit
    fld qword [val_1f64]        ; ST(0)=1.0 → will be ST(1)
    fld qword [val_3f64]        ; ST(0)=3.0, ST(1)=1.0
    fsub st0, st1               ; D8 E1: ST(0) = 3.0 - 1.0 = 2.0
    fstp qword [scratch64]      ; pop result ST(0)=2.0
    mov si, val_2f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    fstp qword [scratch64]      ; discard ST(1)=1.0

    ; FMUL ST0, ST1: 3.0 * 2.0 = 6.0
    fninit
    fld qword [val_2f64]        ; ST(0)=2.0 → will be ST(1)
    fld qword [val_3f64]        ; ST(0)=3.0, ST(1)=2.0
    fmul st0, st1               ; D8 C9: ST(0) = 3.0 * 2.0 = 6.0
    fstp qword [scratch64]      ; pop result ST(0)=6.0
    mov si, val_6f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    fstp qword [scratch64]      ; discard ST(1)=2.0

    ; FDIV ST0, ST1: 6.0 / 2.0 = 3.0
    fninit
    fld qword [val_2f64]        ; ST(0)=2.0 → will be ST(1)
    fld qword [val_6f64]        ; ST(0)=6.0, ST(1)=2.0
    fdiv st0, st1               ; D8 F1: ST(0) = 6.0 / 2.0 = 3.0
    fstp qword [scratch64]      ; pop result ST(0)=3.0
    mov si, val_3f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    fstp qword [scratch64]      ; discard ST(1)=2.0

    ; FADD ST1, ST0 (DC variant): ST(1) += ST(0) → 1.0 + 2.0 = 3.0
    ; Result is in ST(1); pop ST(0) first (discarded), then pop the result.
    fninit
    fld qword [val_1f64]        ; ST(0)=1.0 → will be ST(1)
    fld qword [val_2f64]        ; ST(0)=2.0, ST(1)=1.0
    fadd st1, st0               ; DC C1: ST(1) = 1.0 + 2.0 = 3.0, ST(0) unchanged
    fstp qword [scratch64]      ; pop ST(0)=2.0 (discard)
    fstp qword [scratch64]      ; pop result ST(1)→ST(0)=3.0
    mov si, val_3f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FYL2XP1 — ST(1) * log2(ST(0) + 1), pop
; Push x=1.0 (ST(1)), then y=3.0 (ST(0)).
; Result = 3.0 * log2(1.0+1.0) = 3.0 * 1.0 = 3.0
;=============================================================================
test_fyl2xp1:
    fninit
    fld qword [val_3f64]        ; ST(0)=3.0 → will be ST(1) (y)
    fld qword [val_1f64]        ; ST(0)=1.0 (x), ST(1)=3.0 (y)
    fyl2xp1                     ; ST(0) = 3.0 * log2(1.0+1.0) = 3.0, pop
    fstp qword [scratch64]
    mov si, val_3f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FPREM — partial remainder: ST(0) = ST(0) mod ST(1)
; 5.0 mod 3.0 = 2.0
;=============================================================================
test_fprem:
    fninit
    fld qword [val_3f64]        ; ST(0)=3.0 → will be ST(1) (divisor)
    fld qword [val_5f64]        ; ST(0)=5.0 (dividend), ST(1)=3.0
    fprem                       ; ST(0) = 5.0 - 1*3.0 = 2.0 (ST(1) unchanged)
    fstp qword [scratch64]      ; pop result ST(0)=2.0
    mov si, val_2f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes          ; compare before popping ST(1)
    fstp qword [scratch64]      ; discard ST(1)=3.0
    ret

;=============================================================================
; Test: FSCALE — ST(0) = ST(0) * 2^TRUNC(ST(1))
; 1.0 * 2^3 = 8.0
;=============================================================================
test_fscale:
    fninit
    fld qword [val_3f64]        ; ST(0)=3.0 → will be ST(1) (scale exponent)
    fld qword [val_1f64]        ; ST(0)=1.0 (value), ST(1)=3.0
    fscale                      ; ST(0) = 1.0 * 2^3 = 8.0 (ST(1) unchanged)
    fstp qword [scratch64]      ; pop result ST(0)=8.0
    mov si, val_8f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes          ; compare before popping ST(1)
    fstp qword [scratch64]      ; discard ST(1)=3.0
    ret

;=============================================================================
; Test: FXTRACT — split ST(0) into significand and exponent
; 4.0 = 1.0 * 2^2 → after FXTRACT: ST(0)=1.0 (significand), ST(1)=2.0 (exponent)
;=============================================================================
test_fxtract:
    fninit
    fld qword [val_4f64]        ; ST(0)=4.0
    fxtract                     ; ST(0)=1.0 (significand), ST(1)=2.0 (exponent)
    fstp qword [scratch64]      ; pop significand = 1.0
    mov si, val_1f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    fstp qword [scratch64]      ; pop exponent = 2.0
    mov si, val_2f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; compare_bytes — compare [SI] to [DI] for CX bytes
; Increments pass_count if all match, fail_count if any differ.
;=============================================================================
compare_bytes:
    push si
    push di
    push cx
.loop:
    mov al, [si]
    cmp al, [di]
    jne .fail
    inc si
    inc di
    loop .loop
    inc word [pass_count]
    pop cx
    pop di
    pop si
    ret
.fail:
    pop cx
    pop di
    pop si
    inc word [fail_count]
    ret

;=============================================================================
; Data
;=============================================================================
section .data
; IEEE 754 double-precision constants
val_0f64:    dq 0x0000000000000000  ; 0.0
val_1f64:    dq 0x3FF0000000000000  ; 1.0
val_2f64:    dq 0x4000000000000000  ; 2.0
val_3f64:    dq 0x4008000000000000  ; 3.0
val_4f64:    dq 0x4010000000000000  ; 4.0
val_5f64:    dq 0x4014000000000000  ; 5.0
val_6f64:    dq 0x4018000000000000  ; 6.0
val_8f64:    dq 0x4020000000000000  ; 8.0
val_neg2f64: dq 0xC000000000000000  ; -2.0
val_2_7f64:  dq 0x4005999999999999  ; 2.7 (approx, rounds to 3)
val_pi4f64:  dq 0x3FE921FB54442D18  ; pi/4
; IEEE 754 single-precision constants for memory-operand tests
val_1f32:    dd 0x3F800000          ; 1.0f
val_2f32:    dd 0x40000000          ; 2.0f
val_3f32:    dd 0x40400000          ; 3.0f
val_6f32:    dd 0x40C00000          ; 6.0f

section .bss
scratch64:  resq 1
pass_count: resw 1
fail_count: resw 1
