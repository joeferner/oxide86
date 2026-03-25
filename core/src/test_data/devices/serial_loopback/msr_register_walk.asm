; MSR register walk test (physical loopback, MCR=0)
;
; With no modem lines asserted (MCR=0), the MSR high nibble is 0.
; The UART allows writing to MSR; only bits 3:0 (delta bits) persist and
; are readable - they are cleared on each read (cleared-on-read behaviour).
;
; Sequence for each pattern P:
;   OUT MSR, P       → write pattern
;   IN  AL, MSR      → read back: expect AL == (P & 0x0F), clears delta bits
;   IN  AL, MSR      → read again: expect AL == 0x00 (delta bits cleared)
;
; Exit 0 = pass, Exit 1 = fail.

[CPU 8086]
org 0x0100

COM1_BASE  equ 0x3F8
COM1_MCR   equ 0x3FC
COM1_MSR   equ 0x3FE

start:
    ; Ensure MCR=0 (no modem lines active, no internal loopback)
    mov dx, COM1_MCR
    mov al, 0x00
    out dx, al

    ; Walk through test patterns
    mov al, 0x00
    call check_msr_pattern
    test al, al
    jnz fail

    mov al, 0x55
    call check_msr_pattern
    test al, al
    jnz fail

    mov al, 0xAA
    call check_msr_pattern
    test al, al
    jnz fail

    mov al, 0xFF
    call check_msr_pattern
    test al, al
    jnz fail

    mov al, 0x0F
    call check_msr_pattern
    test al, al
    jnz fail

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

; check_msr_pattern
; Input:  AL = pattern to write to MSR
; Output: AL = 0 success, 1 failure
; Clobbers: AH, BL, DX
check_msr_pattern:
    mov bl, al              ; save pattern

    ; Write pattern to MSR
    mov dx, COM1_MSR
    out dx, al

    ; First read: expect (pattern & 0x0F)
    in al, dx
    and bl, 0x0F            ; expected = pattern & 0x0F
    cmp al, bl
    jne .fail

    ; Second read: delta bits must be cleared
    in al, dx
    test al, 0x0F           ; delta bits must be zero
    jnz .fail

    mov al, 0
    ret

.fail:
    mov al, 1
    ret
