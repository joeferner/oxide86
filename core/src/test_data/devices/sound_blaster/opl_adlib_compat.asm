; opl_adlib_compat.asm - OPL timer detection via AdLib-compat ports (0x388/0x389)
;
; Verifies that the SB16's OPL chip also responds at the AdLib-compat port pair
; when a SoundBlaster (not a standalone Adlib) is registered.
;
; Exit codes: 0=pass, 1=pre-timer status not clear, 2=timer 1 did not fire

[CPU 8086]
org 0x100

start:
    ; Mask both timers and reset status flags
    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x60
    mov dx, 0x389
    out dx, al

    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x80
    mov dx, 0x389
    out dx, al

    ; Read status - must be 0x00
    mov dx, 0x388
    in al, dx
    and al, 0xE0
    jz .timer_start
    mov al, 0x01
    jmp .exit

.timer_start:
    ; Set Timer 1 to 0xFF
    mov al, 0x02
    mov dx, 0x388
    out dx, al
    mov al, 0xFF
    mov dx, 0x389
    out dx, al

    ; Start Timer 1: enable + unmask = 0x21
    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x21
    mov dx, 0x389
    out dx, al

    ; Wait > 640 cycles
    mov cx, 200
.wait:
    nop
    nop
    nop
    nop
    loop .wait

    ; Read status - bits 7 and 6 must both be set (0xC0)
    mov dx, 0x388
    in al, dx
    and al, 0xE0
    cmp al, 0xC0
    je .success
    mov al, 0x02
    jmp .exit

.success:
    ; Stop and clear timers
    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x60
    mov dx, 0x389
    out dx, al
    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x80
    mov dx, 0x389
    out dx, al

    xor al, al

.exit:
    mov ah, 0x4C
    int 0x21
