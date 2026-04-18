; adlib_status_clear.asm - Status register clears when bit 7 of reg 0x04 is written
;
; Sequence:
;   1. Reset and verify status = 0x00
;   2. Start Timer 1 and wait for it to fire (status = 0xC0)
;   3. Write 0x80 to reg 0x04 to clear status flags
;   4. Read status and verify it is 0x00 again
;
; Exit codes: 0 = pass, 1 = pre-timer status not clear,
;             2 = timer 1 did not fire, 3 = status not cleared

[CPU 8086]
org 0x100

start:
    ; Reset both timers and clear status
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

    ; Step 1: verify status is 0x00
    mov dx, 0x388
    in al, dx
    and al, 0xE0
    jz .timer_start
    mov al, 0x01
    jmp .exit

.timer_start:
    ; Set Timer 1 = 0xFF, start it
    mov al, 0x02
    mov dx, 0x388
    out dx, al
    mov al, 0xFF
    mov dx, 0x389
    out dx, al

    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x21
    mov dx, 0x389
    out dx, al

    ; Wait for timer 1 to fire (~640 cycles; wait 2000+)
    mov cx, 200
.wait:
    nop
    nop
    nop
    nop
    loop .wait

    ; Step 2: verify timer fired (status = 0xC0)
    mov dx, 0x388
    in al, dx
    and al, 0xE0
    cmp al, 0xC0
    je .clear
    mov al, 0x02
    jmp .exit

.clear:
    ; Step 3: clear status by writing 0x80 to reg 0x04
    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x80
    mov dx, 0x389
    out dx, al

    ; Step 4: status must now be 0x00
    mov dx, 0x388
    in al, dx
    and al, 0xE0
    jz .success
    mov al, 0x03
    jmp .exit

.success:
    xor al, al

.exit:
    mov ah, 0x4C
    int 0x21
