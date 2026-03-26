; LPT1 loopback plug test
;
; Mirrors the CheckIt diagnostic sequence for LPT1 (base 0x03BC):
;
;   1. Data register readback: write 0x00..0xFF to data port, read back,
;      verify each byte is returned unchanged.
;   2. Control/status loopback: write 0x00 to control and data, read status,
;      verify (status & 0xF8) == 0x30.
;      With the loopback plug and ctrl=0, data=0:
;        bit7 = ctrl[0]     = 0
;        bit6 = ctrl[2]     = 0
;        bit5 = NOT ctrl[1] = 1  → 0x20
;        bit4 = NOT ctrl[3] = 1  → 0x10
;        bit3 = data[0]     = 0
;        bits2:0 = 0x07 (reserved)
;      raw status = 0x37, masked = 0x30  ✓
;
; Exit code: 0 = pass, 1 = fail.

[CPU 8086]
org 0x0100

LPT1_DATA    equ 0x03BC
LPT1_STATUS  equ 0x03BD
LPT1_CTRL    equ 0x03BE

start:
    ; --- Test 1: data register readback (0x00..0xFF) ---
    xor bx, bx          ; BL = current byte value
.data_loop:
    mov al, bl
    mov dx, LPT1_DATA
    out dx, al
    in al, dx
    cmp al, bl
    jne fail
    inc bl
    jnz .data_loop      ; loop until BL wraps from 0xFF to 0x00

    ; --- Test 2: control/status loopback ---
    ; Write 0x00 to control and data
    xor al, al
    mov dx, LPT1_CTRL
    out dx, al
    mov dx, LPT1_DATA
    out dx, al

    ; Read status register and check bits 7:3
    mov dx, LPT1_STATUS
    in al, dx
    and al, 0xF8
    cmp al, 0x30
    jne fail

    ; Pass
    mov ah, 0x4C
    xor al, al
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
