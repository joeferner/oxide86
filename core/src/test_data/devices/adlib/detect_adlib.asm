; detect_adlib.asm - AdLib (OPL2) detection via Timer 1
;
; Standard detection sequence:
;   1. Mask both timers and reset status flags (reg 0x04 = 0x60, then 0x80)
;   2. Read status - must be 0x00
;   3. Set Timer 1 to 0xFF and start it (reg 0x02 = 0xFF, reg 0x04 = 0x21)
;   4. Wait ~400 µs (>640 cycles at 8 MHz for one 80µs tick)
;   5. Read status - bits 7 and 6 must be set (0xC0)
;
; Exit codes: 0 = AdLib detected, 1 = pre-timer status not clear, 2 = timer 1 did not fire

[CPU 8086]
org 0x100

start:
    ; Step 1: mask both timers and reset status flags
    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x60        ; mask timer1 (bit6) and timer2 (bit5)
    mov dx, 0x389
    out dx, al

    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x80        ; reset status flags
    mov dx, 0x389
    out dx, al

    ; Step 2: read status - must be 0x00
    mov dx, 0x388
    in al, dx
    and al, 0xE0
    jz .timer_start
    mov al, 0x01        ; exit 1: pre-timer status not clear
    jmp .exit

.timer_start:
    ; Step 3: set Timer 1 to 0xFF (1 tick = 80 µs to overflow)
    mov al, 0x02
    mov dx, 0x388
    out dx, al
    mov al, 0xFF
    mov dx, 0x389
    out dx, al

    ; Start Timer 1: enable (bit0) + unmask (bit6 clear) = 0x21
    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x21
    mov dx, 0x389
    out dx, al

    ; Step 4: wait > 640 cycles at 8 MHz (200 * ~10 cycles = ~2000 cycles)
    mov cx, 200
.wait:
    nop
    nop
    nop
    nop
    loop .wait

    ; Step 5: read status - bits 7 and 6 must both be set (0xC0)
    mov dx, 0x388
    in al, dx
    and al, 0xE0
    cmp al, 0xC0
    je .success
    mov al, 0x02        ; exit 2: timer 1 did not fire
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
