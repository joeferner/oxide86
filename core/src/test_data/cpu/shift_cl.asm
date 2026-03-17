[CPU 8086]
[ORG 0x100]

; Test 8086 shift/rotate instructions with CL counts, including counts >= 16.
; On the 8086 the shift count is NOT masked — shifting a 16-bit register by 16
; produces 0.  This differs from 286+ and modern x86 which mask CL mod 16/32.
;
; Relevant to the EGALATCH.CK1 LZW bug: the game's E151 routine uses
;   neg cl  /  add cl,16  /  shr bx,cl
; to left-shift BX.  When the original shift is 0 the sequence produces CL=16.
; On real 8086 shr bx,16 → BX=0.  oxide86 must not silently treat it as shr bx,0.

section .text
start:
    mov word [pass_count], 0
    mov word [fail_count], 0

    call test_shr_cl_normal
    call test_shl_cl_normal
    call test_shr_cl_16
    call test_shl_cl_16
    call test_shr_cl_17
    call test_shl_cl_17
    call test_shr_cl_zero
    call test_shl_cl_zero
    call test_shr_byte_cl_8
    call test_shr_byte_cl_9

    ; Exit: 0 if all passed, 1 if any failed
    mov ah, 0x4C
    mov al, 0x00
    cmp word [fail_count], 0
    je .exit
    mov al, 0x01
.exit:
    int 0x21

;=============================================================================
; SHR reg,CL — normal count (CL=4, well under 16)
;=============================================================================
test_shr_cl_normal:
    mov cl, 4
    mov bx, 0x00F0
    shr bx, cl
    cmp bx, 0x000F
    jne .fail
    call print_pass
    ret
.fail:
    call print_fail
    ret

;=============================================================================
; SHL reg,CL — normal count (CL=4)
;=============================================================================
test_shl_cl_normal:
    mov cl, 4
    mov bx, 0x000F
    shl bx, cl
    cmp bx, 0x00F0
    jne .fail
    call print_pass
    ret
.fail:
    call print_fail
    ret

;=============================================================================
; SHR reg,CL — CL=16: all bits shift out, result must be 0
;=============================================================================
test_shr_cl_16:
    mov cl, 16
    mov bx, 0x1234
    shr bx, cl
    cmp bx, 0x0000
    jne .fail
    call print_pass
    ret
.fail:
    call print_fail
    ret

;=============================================================================
; SHL reg,CL — CL=16: all bits shift out, result must be 0
;=============================================================================
test_shl_cl_16:
    mov cl, 16
    mov bx, 0x1234
    shl bx, cl
    cmp bx, 0x0000
    jne .fail
    call print_pass
    ret
.fail:
    call print_fail
    ret

;=============================================================================
; SHR reg,CL — CL=17: same as 16 (still 0)
;=============================================================================
test_shr_cl_17:
    mov cl, 17
    mov bx, 0xFFFF
    shr bx, cl
    cmp bx, 0x0000
    jne .fail
    call print_pass
    ret
.fail:
    call print_fail
    ret

;=============================================================================
; SHL reg,CL — CL=17: same as 16 (still 0)
;=============================================================================
test_shl_cl_17:
    mov cl, 17
    mov bx, 0xFFFF
    shl bx, cl
    cmp bx, 0x0000
    jne .fail
    call print_pass
    ret
.fail:
    call print_fail
    ret

;=============================================================================
; SHR reg,CL — CL=0: no-op, value unchanged
;=============================================================================
test_shr_cl_zero:
    mov cl, 0
    mov bx, 0xABCD
    shr bx, cl
    cmp bx, 0xABCD
    jne .fail
    call print_pass
    ret
.fail:
    call print_fail
    ret

;=============================================================================
; SHL reg,CL — CL=0: no-op, value unchanged
;=============================================================================
test_shl_cl_zero:
    mov cl, 0
    mov bx, 0xABCD
    shl bx, cl
    cmp bx, 0xABCD
    jne .fail
    call print_pass
    ret
.fail:
    call print_fail
    ret

;=============================================================================
; SHR byte,CL — CL=8: all bits of an 8-bit register shift out → 0
;=============================================================================
test_shr_byte_cl_8:
    mov cl, 8
    mov al, 0xFF
    shr al, cl
    cmp al, 0x00
    jne .fail
    call print_pass
    ret
.fail:
    call print_fail
    ret

;=============================================================================
; SHR byte,CL — CL=9: still 0
;=============================================================================
test_shr_byte_cl_9:
    mov cl, 9
    mov al, 0xFF
    shr al, cl
    cmp al, 0x00
    jne .fail
    call print_pass
    ret
.fail:
    call print_fail
    ret

;=============================================================================
; Helpers
;=============================================================================
print_pass:
    inc word [pass_count]
    ret

print_fail:
    inc word [fail_count]
    ret

section .bss
pass_count: resw 1
fail_count: resw 1
