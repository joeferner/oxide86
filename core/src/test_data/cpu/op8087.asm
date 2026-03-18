; op8087.asm - 8087 FPU instruction tests
;
; Tests FLD, FST, FSTP with 32-bit and 64-bit memory operands and
; FLD ST(i) register copies. Loads known IEEE 754 values, stores
; them back, and verifies the bytes are unchanged.
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
val_1f32: dd 0x3F800000             ; 1.0 (IEEE 754 single)
val_2f32: dd 0x40000000             ; 2.0 (IEEE 754 single)
val_1f64: dq 0x3FF0000000000000     ; 1.0 (IEEE 754 double)

section .bss
scratch32a: resd 1
scratch32b: resd 1
scratch64:  resq 1
pass_count: resw 1
fail_count: resw 1
