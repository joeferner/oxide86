; opl_detect.asm - OPL timer detection via SB base port (0x220/0x221)
;
; Same sequence as detect_adlib.asm but using the SB's own OPL port pair.
;
; Exit codes: 0=pass, 1=pre-timer status not clear, 2=timer 1 did not fire

[CPU 8086]
org 0x100

SB_BASE equ 0x220

start:
    ; Mask both timers and reset status flags
    mov al, 0x04
    mov dx, SB_BASE
    out dx, al
    mov al, 0x60
    mov dx, SB_BASE + 1
    out dx, al

    mov al, 0x04
    mov dx, SB_BASE
    out dx, al
    mov al, 0x80
    mov dx, SB_BASE + 1
    out dx, al

    ; Read status - must be 0x00
    mov dx, SB_BASE
    in al, dx
    and al, 0xE0
    jz .timer_start
    mov al, 0x01
    jmp .exit

.timer_start:
    ; Set Timer 1 to 0xFF (1 tick = 80 µs to overflow)
    mov al, 0x02
    mov dx, SB_BASE
    out dx, al
    mov al, 0xFF
    mov dx, SB_BASE + 1
    out dx, al

    ; Start Timer 1: enable (bit0) + unmask (bit6 clear) = 0x21
    mov al, 0x04
    mov dx, SB_BASE
    out dx, al
    mov al, 0x21
    mov dx, SB_BASE + 1
    out dx, al

    ; Wait > 640 cycles at 8 MHz (200 * ~10 cycles = ~2000 cycles)
    mov cx, 200
.wait:
    nop
    nop
    nop
    nop
    loop .wait

    ; Read status - bits 7 and 6 must both be set (0xC0)
    mov dx, SB_BASE
    in al, dx
    and al, 0xE0
    cmp al, 0xC0
    je .success
    mov al, 0x02
    jmp .exit

.success:
    ; Stop and clear timers
    mov al, 0x04
    mov dx, SB_BASE
    out dx, al
    mov al, 0x60
    mov dx, SB_BASE + 1
    out dx, al
    mov al, 0x04
    mov dx, SB_BASE
    out dx, al
    mov al, 0x80
    mov dx, SB_BASE + 1
    out dx, al

    xor al, al

.exit:
    mov ah, 0x4C
    int 0x21
