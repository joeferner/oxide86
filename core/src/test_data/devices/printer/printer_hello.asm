; Printer stub test — sends "Hello, Printer!\r\n" to LPT1 via direct I/O.
;
; LPT1 base address 0x03BC (MDA built-in port, first detected by BIOS):
;   Base+0 (0x03BC) — data register
;   Base+1 (0x03BD) — status register
;   Base+2 (0x03BE) — control register
;
; To strobe a byte to the printer:
;   1. Write byte to data register.
;   2. Write 0x0D to control (Strobe=1, /Init asserted, /Sel-In asserted).
;   3. Write 0x0C to control (Strobe=0 — deassert strobe, keep /Init and /Sel-In).
;
; Also verifies that the status register reports "printer ready" (bit 7 = 1,
; meaning /BUSY is high — not busy).
;
; Exit code: 0 = pass, 1 = fail.

[CPU 8086]
org 0x0100

LPT1_DATA   equ 0x03BC
LPT1_STATUS equ 0x03BD
LPT1_CTRL   equ 0x03BE

CTRL_IDLE   equ 0x0C    ; /Init asserted (bit 2), /Sel-In asserted (bit 3)
CTRL_STROBE equ 0x0D    ; same + Strobe (bit 0)

start:
    ; Verify status register reports ready (bit 7 = 1 = not busy)
    mov dx, LPT1_STATUS
    in al, dx
    test al, 0x80
    jz fail             ; bit 7 clear means busy — unexpected for a stub printer

    ; Send each byte of the message using strobe pulses
    mov si, msg
    mov cx, msg_len
.send_loop:
    lodsb
    mov dx, LPT1_DATA
    out dx, al
    mov dx, LPT1_CTRL
    mov al, CTRL_STROBE
    out dx, al
    mov al, CTRL_IDLE
    out dx, al
    loop .send_loop

    ; Pass
    mov ah, 0x4C
    xor al, al
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

msg     db "Hello, Printer!", 0x0D, 0x0A
msg_len equ $ - msg
