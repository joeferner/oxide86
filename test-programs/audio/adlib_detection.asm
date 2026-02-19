; adlib_detection.asm - AdLib (OPL2/YM3812) sound card detection and tone test
;
; Standard IBM AdLib detection sequence:
;   1. Reset timers via register 4
;   2. Set Timer 1 to 0xFF and start it
;   3. Wait ~100 µs
;   4. Read status port 0x388 - bits 7 and 5 must be set (0xC0)
;
; If AdLib detected, plays a two-note sequence on channel 0.
;
; NOTE: Ports 0x388/0x389 are > 0xFF so they require the DX register form:
;       mov dx, 0x388 / out dx, al  (NOT: out 0x388, al — NASM truncates to 0x88)
;
; Build:  nasm -f bin adlib_detection.asm -o adlib_detection.com
; Run:    cargo run -p emu86-native-gui -- --sound-card adlib adlib_detection.com
;         cargo run -p emu86-native-cli -- --sound-card adlib adlib_detection.com

[CPU 8086]
org 0x100

; ─── AdLib detection ──────────────────────────────────────────────────────────

    ; Step 1: reset both timers (reg 4 = 0x60)
    mov al, 0x04
    mov dx, 0x388
    out dx, al          ; address = timer control register
    mov al, 0x60
    mov dx, 0x389
    out dx, al          ; mask timer 1 & 2, reset flags

    ; Step 2: reset IRQ status (reg 4 = 0x80)
    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x80
    mov dx, 0x389
    out dx, al

    ; Step 3: read status - should be 0x00 before timer fires
    mov dx, 0x388
    in al, dx

    ; Step 4: set Timer 1 to 0xFF (expires after ~320 µs)
    mov al, 0x02
    mov dx, 0x388
    out dx, al
    mov al, 0xFF
    mov dx, 0x389
    out dx, al

    ; Step 5: start Timer 1 (reg 4 = 0x21: start=1, unmask=1)
    mov al, 0x04
    mov dx, 0x388
    out dx, al
    mov al, 0x21
    mov dx, 0x389
    out dx, al

    ; Step 6: wait ~400 µs  (roughly 1900 cycles at 4.77 MHz)
    mov cx, 200
.wait:
    nop
    nop
    nop
    nop
    loop .wait

    ; Step 7: read status - bits 7 and 5 must be set for real AdLib
    mov dx, 0x388
    in al, dx
    and al, 0xE0        ; mask to bits 7,6,5
    cmp al, 0xC0        ; expect bit 7 (IRQ) + bit 5 (Timer 1 expired)
    jne .not_found

    ; Step 8: stop and reset timers
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

    ; ─── AdLib found ──────────────────────────────────────────────────────────
    mov dx, msg_found
    mov ah, 0x09
    int 0x21

    ; Enable waveform select (reg 0x01 bit 5)
    mov al, 0x01
    mov dx, 0x388
    out dx, al
    mov al, 0x20
    mov dx, 0x389
    out dx, al

    ; ── Set up operator slot 0 (modulator for channel 0) ──
    ; reg 0x20: AM=0 VIB=0 EG=1 KSR=0 MULT=1 → sustained, multiply x1
    call adlib_write_reg
    db 0x20, 0x21

    ; reg 0x40: KSL=0 TL=16 (moderate volume, range 0-63, lower = louder)
    call adlib_write_reg
    db 0x40, 0x10

    ; reg 0x60: AR=15 DR=0 (instant attack, no decay)
    call adlib_write_reg
    db 0x60, 0xF0

    ; reg 0x80: SL=0 RR=7 (sustain level 0, release rate 7)
    call adlib_write_reg
    db 0x80, 0x07

    ; reg 0xE0: waveform = 0 (sine)
    call adlib_write_reg
    db 0xE0, 0x00

    ; ── Set up operator slot 3 (carrier for channel 0) ──
    ; reg 0x23: AM=0 VIB=0 EG=1 KSR=0 MULT=1
    call adlib_write_reg
    db 0x23, 0x21

    ; reg 0x43: KSL=0 TL=0 (full volume for carrier)
    call adlib_write_reg
    db 0x43, 0x00

    ; reg 0x63: AR=15 DR=0
    call adlib_write_reg
    db 0x63, 0xF0

    ; reg 0x83: SL=0 RR=7
    call adlib_write_reg
    db 0x83, 0x07

    ; reg 0xE3: waveform = 0 (sine)
    call adlib_write_reg
    db 0xE3, 0x00

    ; ── Channel 0 feedback/algorithm: FM (algo=0), feedback=4 ──
    call adlib_write_reg
    db 0xC0, 0x08

    ; ── Play note 1: A4 (440 Hz), block=4, fnum=0x244 ──
    ; reg 0xA0: fnum low byte = 0x44
    call adlib_write_reg
    db 0xA0, 0x44

    ; reg 0xB0: key_on=1 block=4 fnum_hi=0x02 → 0x20 | 0x10 | 0x02 = 0x32
    call adlib_write_reg
    db 0xB0, 0x32

    ; Wait ~0.5 seconds (busy delay)
    call delay_long

    ; ── Play note 2: D5 (~587 Hz), block=4, fnum=0x308 ──
    ; reg 0xA0: fnum low byte = 0x08
    call adlib_write_reg
    db 0xA0, 0x08

    ; reg 0xB0: key_on=1 block=4 fnum_hi=0x03 → 0x20 | 0x10 | 0x03 = 0x33
    call adlib_write_reg
    db 0xB0, 0x33

    ; Wait ~0.5 seconds
    call delay_long

    ; ── Key off (silence channel 0) ──
    ; Clear key_on bit in B0
    call adlib_write_reg
    db 0xB0, 0x13       ; same block/fnum but key_on=0

    jmp .done

.not_found:
    mov dx, msg_not_found
    mov ah, 0x09
    int 0x21

.done:
    ; Exit
    mov ah, 0x4C
    xor al, al
    int 0x21

; ─── Subroutine: adlib_write_reg ──────────────────────────────────────────────
; Reads two inline bytes after CALL:  [reg_addr, reg_value]
; Uses AX, BX, DX. Caller IP is updated past the two data bytes.
adlib_write_reg:
    pop bx              ; BX = return address (points at inline data bytes)
    mov al, [bx]        ; AL = register address
    mov dx, 0x388
    out dx, al
    ; Short delay (OPL2 needs ~3.3µs between address write and data write)
    nop
    nop
    nop
    mov al, [bx+1]      ; AL = data value
    mov dx, 0x389
    out dx, al
    ; Short delay (~23µs recommended after data write)
    mov cx, 4
.reg_delay:
    nop
    loop .reg_delay
    add bx, 2           ; skip past inline data bytes
    push bx             ; push updated return address
    ret

; ─── Subroutine: delay_long ───────────────────────────────────────────────────
; Busy-wait ~0.5 seconds at 4.77 MHz (roughly 2.38 million cycles → ~30 outer loops)
delay_long:
    push cx
    push dx
    mov dx, 30
.outer:
    mov cx, 0xFFFF
.inner:
    nop
    loop .inner
    dec dx
    jnz .outer
    pop dx
    pop cx
    ret

; ─── Data ─────────────────────────────────────────────────────────────────────
msg_found     db 'AdLib OPL2 detected - playing two notes...', 0x0D, 0x0A, '$'
msg_not_found db 'AdLib not found (status check failed)', 0x0D, 0x0A, '$'
