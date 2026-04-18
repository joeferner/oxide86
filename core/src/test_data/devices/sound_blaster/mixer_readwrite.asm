; mixer_readwrite.asm — Mixer register round-trip
;
; 1. Write 0x22 to mixer index (master volume)
; 2. Write 0xCC to data port
; 3. Re-select 0x22, read data port
; 4. Verify returned value is 0xCC
;
; Also verifies IRQ config register 0x80 reads back after write.
;
; Exit: 0=pass, 1=master vol mismatch, 2=IRQ reg mismatch

[CPU 8086]
org 0x100

SB_BASE equ 0x220

start:
    ; Write master volume
    mov dx, SB_BASE + 4
    mov al, 0x22
    out dx, al
    mov dx, SB_BASE + 5
    mov al, 0xCC
    out dx, al

    ; Read back
    mov dx, SB_BASE + 4
    mov al, 0x22
    out dx, al
    mov dx, SB_BASE + 5
    in al, dx
    cmp al, 0xCC
    je .irq_test
    mov al, 0x01
    jmp .exit

.irq_test:
    ; Write IRQ select: IRQ5 = bit 2
    mov dx, SB_BASE + 4
    mov al, 0x80
    out dx, al
    mov dx, SB_BASE + 5
    mov al, 0x04
    out dx, al

    ; Read back
    mov dx, SB_BASE + 4
    mov al, 0x80
    out dx, al
    mov dx, SB_BASE + 5
    in al, dx
    cmp al, 0x04
    je .pass
    mov al, 0x02
    jmp .exit

.pass:
    xor al, al
.exit:
    mov ah, 0x4C
    int 0x21
