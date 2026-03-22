; op8087_int64.asm - 8087 FPU 64-bit integer precision tests
;
; Verifies that FILD/FISTP handle large 64-bit integers exactly.  These
; values exceed f64's 2^53 integer precision limit, so an emulator that
; stores the FPU stack as f64 will silently round them and produce wrong
; results.  The emulator must use 80-bit extended precision (F80) internally
; to pass these tests.
;
; The motivating failure: checkit.exe's 8087 arithmetic test loads
;   A = 0x5555555555555555  (+6,148,914,691,236,517,205)
;   B = 0xAAAAAAAAAAAAAAAA  (−6,148,914,691,236,517,206 signed)
; adds them with FADDP, and expects −1.  An f64-based emulator rounds both
; to the same magnitude → A+B = 0 instead of −1.
;
; Test cases:
;   1. checkit scenario:  A + B = −1
;   2. i64::MAX round-trip  (0x7FFFFFFFFFFFFFFF)
;   3. i64::MIN round-trip  (0x8000000000000000)
;   4. 2^53+1 round-trip    (0x0020000000000001 — first integer f64 cannot hold)
;   5. −(2^53+1) round-trip (0xFFDFFFFFFFFFFFFF)
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

    call test_checkit_add
    call test_i64_max_roundtrip
    call test_i64_min_roundtrip
    call test_above_2_53
    call test_neg_above_2_53

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
; Test 1: the exact checkit.exe scenario
; FILD A, FILD B, FADDP, FISTP → expect −1
;=============================================================================
test_checkit_add:
    fninit
    fild qword [val_A]          ; ST(0) = +6,148,914,691,236,517,205
    fild qword [val_B]          ; ST(0) = −6,148,914,691,236,517,206, ST(1) = A
    faddp                       ; ST(0) = A + B = −1, pops one
    fistp qword [scratch64]
    mov si, val_neg1_i64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test 2: i64::MAX round-trip (0x7FFFFFFFFFFFFFFF = 9,223,372,036,854,775,807)
;=============================================================================
test_i64_max_roundtrip:
    fninit
    fild qword [val_i64_max]
    fistp qword [scratch64]
    mov si, val_i64_max
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test 3: i64::MIN round-trip (0x8000000000000000 = −9,223,372,036,854,775,808)
;=============================================================================
test_i64_min_roundtrip:
    fninit
    fild qword [val_i64_min]
    fistp qword [scratch64]
    mov si, val_i64_min
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test 4: 2^53 + 1 = 9,007,199,254,740,993 (0x0020000000000001)
; This is the first positive integer that f64 cannot represent exactly
; (f64 has 53-bit mantissa, so 2^53 is the last exact integer boundary).
;=============================================================================
test_above_2_53:
    fninit
    fild qword [val_2_53_plus_1]
    fistp qword [scratch64]
    mov si, val_2_53_plus_1
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test 5: −(2^53 + 1) = −9,007,199,254,740,993 (0xFFDFFFFFFFFFFFFF)
;=============================================================================
test_neg_above_2_53:
    fninit
    fild qword [val_neg_2_53_plus_1]
    fistp qword [scratch64]
    mov si, val_neg_2_53_plus_1
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
; Data — all 64-bit integers stored little-endian
;=============================================================================
section .data

; A = 0x5555555555555555  (+6,148,914,691,236,517,205)
val_A:               dq 0x5555555555555555

; B = 0xAAAAAAAAAAAAAAAA  (−6,148,914,691,236,517,206 signed)
val_B:               dq 0xAAAAAAAAAAAAAAAA

; Expected A+B = −1
val_neg1_i64:        dq 0xFFFFFFFFFFFFFFFF

; i64::MAX = 9,223,372,036,854,775,807
val_i64_max:         dq 0x7FFFFFFFFFFFFFFF

; i64::MIN = −9,223,372,036,854,775,808
val_i64_min:         dq 0x8000000000000000

; 2^53 + 1 = 9,007,199,254,740,993
val_2_53_plus_1:     dq 0x0020000000000001

; −(2^53 + 1) = −9,007,199,254,740,993
val_neg_2_53_plus_1: dq 0xFFDFFFFFFFFFFFFF

section .bss
scratch64:  resq 1
pass_count: resw 1
fail_count: resw 1
