; op8087_compare.asm - 8087 FCOM / FTST instruction tests
;
; Tests FTST (compare ST(0) against 0) and FCOM (compare ST(0) against ST(1)
; or a memory operand). After each comparison, FNSTSW AX + SAHF transfers the
; FPU condition codes into CPU flags so standard Jcc branches can be used.
;
; Flag mapping via FNSTSW AX + SAHF:
;   C0 (SW bit 8)  -> AH bit 0 -> CF   (set when ST < operand or unordered)
;   C2 (SW bit 10) -> AH bit 2 -> PF   (set when unordered)
;   C3 (SW bit 14) -> AH bit 6 -> ZF   (set when ST = operand)
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

    call test_ftst
    call test_fcom_reg
    call test_fcom_mem

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
; test_ftst — compare ST(0) against +0.0
;=============================================================================
test_ftst:
    ; Case 1: positive value: ST(0)=1.0 > 0.0 => CF=0, ZF=0
    fninit
    fld dword [val_pos]
    db 0xD9, 0xE4       ; FTST
    db 0xDF, 0xE0       ; FNSTSW AX
    sahf
    jb  .fail           ; CF=1 means less than, unexpected
    je  .fail           ; ZF=1 means equal, unexpected
    inc word [pass_count]
    fstp dword [scratch]

    ; Case 2: negative value: ST(0)=-1.0 < 0.0 => CF=1, ZF=0
    fninit
    fld dword [val_neg]
    db 0xD9, 0xE4       ; FTST
    db 0xDF, 0xE0       ; FNSTSW AX
    sahf
    jnb .fail           ; CF=0 means not less than, unexpected
    inc word [pass_count]
    fstp dword [scratch]

    ; Case 3: zero: ST(0)=0.0 == 0.0 => ZF=1, CF=0
    fninit
    fld dword [val_zero]
    db 0xD9, 0xE4       ; FTST
    db 0xDF, 0xE0       ; FNSTSW AX
    sahf
    jne .fail           ; ZF=0 means not equal, unexpected
    jb  .fail           ; CF=1 means less than, unexpected
    inc word [pass_count]
    fstp dword [scratch]

    ret
.fail:
    inc word [fail_count]
    ret

;=============================================================================
; test_fcom_reg — FCOM ST(0) against ST(1)
;=============================================================================
test_fcom_reg:
    ; Case 4: 1.0 < 2.0 => CF=1, ZF=0
    fninit
    fld dword [val_two]     ; ST(0)=2.0
    fld dword [val_pos]     ; ST(0)=1.0, ST(1)=2.0
    db 0xD8, 0xD1           ; FCOM ST(1)
    db 0xDF, 0xE0           ; FNSTSW AX
    sahf
    jnb .fail               ; CF=0 means not less than, unexpected
    inc word [pass_count]
    fstp dword [scratch]    ; pop 1.0
    fstp dword [scratch]    ; pop 2.0

    ; Case 5: 2.0 > 1.0 => CF=0, ZF=0
    fninit
    fld dword [val_pos]     ; ST(0)=1.0
    fld dword [val_two]     ; ST(0)=2.0, ST(1)=1.0
    db 0xD8, 0xD1           ; FCOM ST(1)
    db 0xDF, 0xE0           ; FNSTSW AX
    sahf
    jb  .fail               ; CF=1 means less than, unexpected
    je  .fail               ; ZF=1 means equal, unexpected
    inc word [pass_count]
    fstp dword [scratch]    ; pop 2.0
    fstp dword [scratch]    ; pop 1.0

    ; Case 6: 1.0 == 1.0 => ZF=1, CF=0
    fninit
    fld dword [val_pos]     ; ST(0)=1.0
    fld dword [val_pos]     ; ST(0)=1.0, ST(1)=1.0
    db 0xD8, 0xD1           ; FCOM ST(1)
    db 0xDF, 0xE0           ; FNSTSW AX
    sahf
    jne .fail               ; ZF=0 means not equal, unexpected
    jb  .fail               ; CF=1 means less than, unexpected
    inc word [pass_count]
    fstp dword [scratch]
    fstp dword [scratch]

    ret
.fail:
    inc word [fail_count]
    ret

;=============================================================================
; test_fcom_mem — FCOM ST(0) against a 32-bit memory operand
;=============================================================================
test_fcom_mem:
    ; Case 7: ST(0)=1.0 < [val_two]=2.0 => CF=1, ZF=0
    fninit
    fld dword [val_pos]             ; ST(0)=1.0
    fcom dword [val_two]
    db 0xDF, 0xE0                   ; FNSTSW AX
    sahf
    jnb .fail                       ; CF=0 means not less than, unexpected
    inc word [pass_count]
    fstp dword [scratch]

    ; Case 8: ST(0)=2.0 > [val_pos]=1.0 => CF=0, ZF=0
    fninit
    fld dword [val_two]             ; ST(0)=2.0
    fcom dword [val_pos]
    db 0xDF, 0xE0                   ; FNSTSW AX
    sahf
    jb  .fail                       ; CF=1 means less than, unexpected
    je  .fail                       ; ZF=1 means equal, unexpected
    inc word [pass_count]
    fstp dword [scratch]

    ret
.fail:
    inc word [fail_count]
    ret

;=============================================================================
; Data
;=============================================================================
section .data
val_pos:  dd 0x3F800000     ;  1.0f32
val_neg:  dd 0xBF800000     ; -1.0f32
val_zero: dd 0x00000000     ;  0.0f32
val_two:  dd 0x40000000     ;  2.0f32

section .bss
scratch:    resd 1
pass_count: resw 1
fail_count: resw 1
