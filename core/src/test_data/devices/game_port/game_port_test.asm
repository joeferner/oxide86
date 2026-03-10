; Game port (0x201) test:
;   Rust sets up the game port before running this program:
;     - Axis X1 = 0   (times out immediately after one-shot write)
;     - Axis Y1 = 200 (stays high for ~17600 cycles at 8 MHz -- well beyond a
;                      single IN instruction's latency)
;     - Button 1 = pressed, Button 2 = released
;
; Port 0x201 > 0xFF so we must use the DX-register form of IN/OUT.
;
; Test 1: Before firing the one-shot, all timing bits (0-3) are 0.
; Test 2: Button 1 bit (4) = 0 (pressed); Button 2 bit (5) = 1 (released).
; Test 3: After writing to 0x201, axis X1 bit (0) = 0 (timed out, axis=0).
; Test 4: After writing to 0x201, axis Y1 bit (1) = 1 (still timing, axis=200).

[CPU 8086]
org 0x0100

GAME_PORT equ 0x201

start:
    mov dx, GAME_PORT

    ; ── Test 1 & 2: Read status without firing one-shot ──────────────────
    in al, dx

    ; Bits 0-3: timing bits must all be 0 (one-shot not yet triggered)
    test al, 0x0F
    jnz fail

    ; Bit 4 = button 1: 0 = pressed
    test al, 0x10
    jnz fail

    ; Bit 5 = button 2: 1 = released
    test al, 0x20
    jz fail

    ; ── Test 3 & 4: Fire one-shot, check axis timing ──────────────────────
    out dx, al              ; any write triggers one-shot
    in  al, dx

    ; Bit 0 = axis X1: value 0 → cycles_needed = 0, already expired
    test al, 0x01
    jnz fail

    ; Bit 1 = axis Y1: value 200 → needs ~17600 cycles @ 8 MHz, still timing
    test al, 0x02
    jz fail

    ; ── Pass ──────────────────────────────────────────────────────────────
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
