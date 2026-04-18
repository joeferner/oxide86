; adlib_timer2.asm - AdLib Timer 2 fires and sets status bits 7 and 5
;
; Timer 2 ticks every 320 µs (vs 80 µs for Timer 1).
; At 8 MHz: 320e-6 * 8_000_000 = 2560 cycles per tick.
; With value 0xFF the timer overflows after 1 tick (2560 cycles).
; We wait ~4000 cycles to be safe.
;
; After Timer 2 fires the status register should have bits 7 and 5 set (0xA0).
;
; Exit codes: 0 = pass, 1 = pre-timer status not clear, 2 = timer 2 did not fire

[CPU 8086]
org 0x100

start:
    ; Reset both timers and clear status flags
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

    ; Verify status is clear
    mov dx, 0x388
    in al, dx
    and al, 0xE0
    jz .timer_start
    mov al, 0x01
    jmp .exit

.timer_start:
    ; Set Timer 2 to 0xFF (overflows after 1 tick = 2560 cycles at 8 MHz)
    mov al, 0x03
    mov dx, 0x388
    out dx, al
    mov al, 0xFF
    mov dx, 0x389
    out dx, al

    ; Start Timer 2: enable (bit1) + unmask (bit5 clear) = 0x02
    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x02
    mov dx, 0x389
    out dx, al

    ; Wait > 2560 cycles (400 * ~10 cycles = ~4000 cycles)
    mov cx, 400
.wait:
    nop
    nop
    nop
    nop
    loop .wait

    ; Status must have bits 7 and 5 set (0xA0) for Timer 2 overflow
    mov dx, 0x388
    in al, dx
    and al, 0xE0
    cmp al, 0xA0
    je .success
    mov al, 0x02
    jmp .exit

.success:
    ; Stop and clear
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
