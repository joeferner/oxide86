; op8087_load_store.asm - 8087 FPU load/store instruction tests
;
; Tests FLD, FST, FSTP, FLDZ, FLD1, FILD, FIST, FISTP, FBLD, FBSTP,
; FLDPI, FLDL2E, FLDLN2, FLDL2T, FLDLG2, FLD ST(i) register copies,
; FLD/FSTP m80 (tword), FILD/FISTP m64.
; Loads known values, stores them back, and verifies the bytes are unchanged.
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

    call test_fld_fstp_m32
    call test_fld_fst_m32
    call test_fld_fstp_m64
    call test_fld_st
    call test_fldz_fld1
    call test_fild_fist
    call test_fbld_fbstp
    call test_fld_constants
    call test_fld_constants_extra
    call test_fld_m80
    call test_fild_fistp_m64

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
; Test: FLD m32 + FSTP m32
; Load 1.0f32, store via FSTP, verify bytes are unchanged.
;=============================================================================
test_fld_fstp_m32:
    fninit
    fld dword [val_1f32]
    fstp dword [scratch32a]
    mov si, val_1f32
    mov di, scratch32a
    mov cx, 4
    call compare_bytes
    ret

;=============================================================================
; Test: FLD m32 + FST m32 (no pop) + FSTP m32
; Load 2.0f32, store once via FST (no pop), once via FSTP,
; verify both destinations match the original.
;=============================================================================
test_fld_fst_m32:
    fninit
    fld dword [val_2f32]
    fst dword [scratch32a]
    fstp dword [scratch32b]
    mov si, val_2f32
    mov di, scratch32a
    mov cx, 4
    call compare_bytes
    mov si, val_2f32
    mov di, scratch32b
    mov cx, 4
    call compare_bytes
    ret

;=============================================================================
; Test: FLD m64 + FSTP m64
; Load 1.0f64, store via FSTP, verify bytes are unchanged.
;=============================================================================
test_fld_fstp_m64:
    fninit
    fld qword [val_1f64]
    fstp qword [scratch64]
    mov si, val_1f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FLD ST(i) — duplicate ST(0) via register copy
; Load 1.0f32, push a copy via FLD ST(0), pop both via FSTP,
; verify both results equal the original.
;=============================================================================
test_fld_st:
    fninit
    fld dword [val_1f32]    ; ST(0) = 1.0f32
    fld st0                 ; ST(0) = copy, ST(1) = original
    fstp dword [scratch32a] ; store copy
    fstp dword [scratch32b] ; store original
    mov si, val_1f32
    mov di, scratch32a
    mov cx, 4
    call compare_bytes
    mov si, val_1f32
    mov di, scratch32b
    mov cx, 4
    call compare_bytes
    ret

;=============================================================================
; Test: FLDZ / FLD1 — load the FPU constant 0.0 and 1.0
; Verify by storing as 32-bit float and comparing bytes.
;=============================================================================
test_fldz_fld1:
    ; FLDZ → FSTP dword → expect 0.0 = 0x00000000
    fninit
    fldz
    fstp dword [scratch32a]
    mov si, val_0f32
    mov di, scratch32a
    mov cx, 4
    call compare_bytes

    ; FLD1 → FSTP dword → expect 1.0 = 0x3F800000
    fninit
    fld1
    fstp dword [scratch32a]
    mov si, val_1f32
    mov di, scratch32a
    mov cx, 4
    call compare_bytes
    ret

;=============================================================================
; Test: FILD / FIST / FISTP — integer load and store
; Load 16-bit integer 42, store once via FIST (no pop) and once via FISTP,
; verify both scratch values match the original.
;=============================================================================
test_fild_fist:
    fninit
    fild word [val_int16]       ; ST(0) = 42.0
    fist word [scratch16a]      ; store without pop
    fistp word [scratch16b]     ; store and pop
    mov si, val_int16
    mov di, scratch16a
    mov cx, 2
    call compare_bytes
    mov si, val_int16
    mov di, scratch16b
    mov cx, 2
    call compare_bytes
    ret

;=============================================================================
; Test: FBLD / FBSTP — packed BCD load and store
; Load 10-byte packed BCD (value 12345), store back via FBSTP,
; verify all 10 bytes are unchanged.
;=============================================================================
test_fbld_fbstp:
    fninit
    fbld [val_bcd]
    fbstp [scratch_bcd]
    mov si, val_bcd
    mov di, scratch_bcd
    mov cx, 10
    call compare_bytes
    ret

;=============================================================================
; Test: FLDPI / FLDL2E / FLDLN2 — transcendental constants
; Each constant is stored as a 64-bit double and compared against
; the known IEEE 754 double representation of that constant.
;=============================================================================
test_fld_constants:
    ; FLDPI → FSTP qword → expect pi = 0x400921FB54442D18
    fninit
    fldpi
    fstp qword [scratch64]
    mov si, val_pi_f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes

    ; FLDL2E → FSTP qword → expect log2(e) = 0x3FF71547652B82FE
    fninit
    fldl2e
    fstp qword [scratch64]
    mov si, val_l2e_f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes

    ; FLDLN2 → FSTP qword → expect ln(2) = 0x3FE62E42FEFA39EF
    fninit
    fldln2
    fstp qword [scratch64]
    mov si, val_ln2_f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FLDL2T / FLDLG2 — additional transcendental constants
;   FLDL2T: log2(10)  = 0x400A934F0979A371
;   FLDLG2: log10(2)  = 0x3FD34413509F79FF
;=============================================================================
test_fld_constants_extra:
    ; FLDL2T → FSTP qword → expect log2(10)
    fninit
    fldl2t
    fstp qword [scratch64]
    mov si, val_l2t_f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes

    ; FLDLG2 → FSTP qword → expect log10(2)
    fninit
    fldlg2
    fstp qword [scratch64]
    mov si, val_lg2_f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes
    ret

;=============================================================================
; Test: FLD tword / FSTP tword — 80-bit extended precision load and store
; Load 1.0 from a known 10-byte f80 buffer, store as f64, compare.
; Then load 1.0 from f64, store as f80, compare bytes.
;=============================================================================
test_fld_m80:
    ; FLD tword → FSTP qword
    fninit
    fld tword [val_1f80]
    fstp qword [scratch64]
    mov si, val_1f64
    mov di, scratch64
    mov cx, 8
    call compare_bytes

    ; FLD qword → FSTP tword → compare bytes to val_1f80
    fninit
    fld qword [val_1f64]
    fstp tword [scratch80]
    mov si, val_1f80
    mov di, scratch80
    mov cx, 10
    call compare_bytes
    ret

;=============================================================================
; Test: FILD m64 / FISTP m64 — 64-bit integer load and store round-trip
;=============================================================================
test_fild_fistp_m64:
    fninit
    fild qword [val_int64]
    fistp qword [scratch_int64]
    mov si, val_int64
    mov di, scratch_int64
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
val_1f32:    dd 0x3F800000             ; 1.0 (IEEE 754 single)
val_2f32:    dd 0x40000000             ; 2.0 (IEEE 754 single)
val_0f32:    dd 0x00000000             ; 0.0 (IEEE 754 single)
val_1f64:    dq 0x3FF0000000000000     ; 1.0 (IEEE 754 double)
val_int16:   dw 42                     ; integer 42
; Packed BCD 12345: sign=0x00, digits pairs 01|23|45
val_bcd:     db 0x45, 0x23, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
val_pi_f64:  dq 0x400921FB54442D18     ; pi  (IEEE 754 double)
val_l2e_f64: dq 0x3FF71547652B82FE    ; log2(e) (IEEE 754 double)
val_ln2_f64: dq 0x3FE62E42FEFA39EF    ; ln(2) (IEEE 754 double)
val_l2t_f64: dq 0x400A934F0979A371    ; log2(10) (IEEE 754 double)
val_lg2_f64: dq 0x3FD34413509F79FF    ; log10(2) (IEEE 754 double)
; 1.0 in 80-bit extended (tword): mantissa=0x8000000000000000, exp+sign=0x3FFF
val_1f80:    db 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xFF, 0x3F
val_int64:   dq 42                    ; 64-bit integer 42

section .bss
scratch32a:   resd 1
scratch32b:   resd 1
scratch64:    resq 1
scratch16a:   resw 1
scratch16b:   resw 1
scratch_bcd:  resb 10
scratch80:    resb 10
scratch_int64: resq 1
pass_count:   resw 1
fail_count:   resw 1
