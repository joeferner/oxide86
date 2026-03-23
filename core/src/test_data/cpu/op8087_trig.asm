; op8087_trig.asm - 8087 FPTAN / FPATAN precision tests
;
; Compares full 80-bit (tword) results against known-correct bit patterns so
; that any rounding error in the last mantissa bit is detected.
;
; Tests:
;   FPTAN(0.0)    → tan=0.0, top=1.0
;   FPTAN(pi/4)   → tan=1.0, top=1.0
;   FPATAN(0/1)   → 0.0
;   FPATAN(1/1)   → pi/4
;   FPATAN(1/0)   → pi/2
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

    call test_fptan_zero
    call test_fptan_pi4
    call test_fpatan_zero
    call test_fpatan_one_one
    call test_fpatan_one_zero

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
; Test: FPTAN(0.0)
; ST(0) = 0.0 → after FPTAN: ST(0)=1.0, ST(1)=tan(0)=0.0
;=============================================================================
test_fptan_zero:
    fninit
    fldz                            ; ST(0) = 0.0
    fptan                           ; ST(0)=1.0, ST(1)=0.0

    ; verify ST(0) = 1.0
    fstp tword [scratch10]
    mov si, val_1f80
    mov di, scratch10
    mov cx, 10
    call compare_bytes

    ; verify ST(1)→ST(0) = 0.0
    fstp tword [scratch10]
    mov si, val_0f80
    mov di, scratch10
    mov cx, 10
    call compare_bytes

    ret

;=============================================================================
; Test: FPTAN(pi/4)
; tan(pi/4) = 1.0 (exact)
;=============================================================================
test_fptan_pi4:
    fninit
    fld tword [val_pi4f80]          ; ST(0) = pi/4
    fptan                           ; ST(0)=1.0, ST(1)=tan(pi/4)=1.0

    ; verify ST(0) = 1.0 (the pushed constant)
    fstp tword [scratch10]
    mov si, val_1f80
    mov di, scratch10
    mov cx, 10
    call compare_bytes

    ; verify tan result = 1.0
    fstp tword [scratch10]
    mov si, val_1f80
    mov di, scratch10
    mov cx, 10
    call compare_bytes

    ret

;=============================================================================
; Test: FPATAN — atan2(0.0, 1.0) = 0.0
; Push x=1.0, push y=0.0 → ST(0)=y=0.0, ST(1)=x=1.0
; FPATAN: result = atan(ST(1)/ST(0)) = atan(1.0/0.0)? No — atan(y,x)=atan(0/1)=0
;
; Intel: FPATAN computes atan(ST(1)/ST(0)), i.e. atan(y/x) where ST(1)=y, ST(0)=x
;=============================================================================
test_fpatan_zero:
    fninit
    fldz                            ; ST(0)=0.0  (will become y=ST(1))
    fld1                            ; ST(0)=1.0  (x), ST(1)=0.0 (y)
    fpatan                          ; result = atan(y/x) = atan(0.0/1.0) = 0.0, pop
    fstp tword [scratch10]
    mov si, val_0f80
    mov di, scratch10
    mov cx, 10
    call compare_bytes
    ret

;=============================================================================
; Test: FPATAN — atan2(1.0, 1.0) = pi/4
; Push x=1.0, push y=1.0 → ST(0)=y=1.0, ST(1)=x=1.0
;=============================================================================
test_fpatan_one_one:
    fninit
    fld1                            ; ST(0)=1.0  (will become x=ST(1))
    fld1                            ; ST(0)=1.0  (y), ST(1)=1.0 (x)
    fpatan                          ; result = atan(1.0 / 1.0) = pi/4, pop
    fstp tword [scratch10]
    mov si, val_pi4f80
    mov di, scratch10
    mov cx, 10
    call compare_bytes
    ret

;=============================================================================
; Test: FPATAN — atan2(1.0, 0.0) = pi/2
; Push x=0.0, push y=1.0 → ST(0)=y=1.0, ST(1)=x=0.0
;=============================================================================
test_fpatan_one_zero:
    fninit
    fld1                            ; ST(0)=1.0  (will become y=ST(1))
    fldz                            ; ST(0)=0.0  (x), ST(1)=1.0 (y)
    fpatan                          ; result = atan(y/x) = atan(1.0/0.0) → pi/2, pop
    fstp tword [scratch10]
    mov si, val_pi2f80
    mov di, scratch10
    mov cx, 10
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
; Data — 80-bit (tword) constants
; Layout: 8 bytes mantissa (LE) + 2 bytes sign|exp (LE)
;
;   sign|exp field: sign at bit15, biased exp in bits 14..0 (bias = 16383)
;
;   0.0   : mant=0x0000000000000000, exp=0x0000 → 00 00 00 00 00 00 00 00 00 00
;   1.0   : mant=0x8000000000000000, exp=0x3FFF → 00 00 00 00 00 00 00 80 FF 3F
;   pi/4  : mant=0xC90FDAA22168C235, exp=0x3FFE → 35 C2 68 21 A2 DA 0F C9 FE 3F
;   pi/2  : mant=0xC90FDAA22168C235, exp=0x3FFF → 35 C2 68 21 A2 DA 0F C9 FF 3F
;=============================================================================
section .data

val_0f80:   db 0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00
val_1f80:   db 0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x80,0xFF,0x3F
val_pi4f80: db 0x35,0xC2,0x68,0x21,0xA2,0xDA,0x0F,0xC9,0xFE,0x3F
val_pi2f80: db 0x35,0xC2,0x68,0x21,0xA2,0xDA,0x0F,0xC9,0xFF,0x3F

section .bss
scratch10:  resb 10
pass_count: resw 1
fail_count: resw 1
