; simple_beep.asm - Simple 1000 Hz beep test

[CPU 8086]
org 0x100

; Set PIT Channel 2 to Mode 3 (square wave), 1000 Hz
mov al, 0xB6        ; Channel 2, LSB+MSB, Mode 3, Binary
out 0x43, al

; Set count = 1193 for ~1000 Hz (PIT_FREQUENCY_HZ / 1193 ≈ 1000)
mov ax, 1193
out 0x42, al        ; LSB
mov al, ah
out 0x42, al        ; MSB

; Enable speaker (set bits 0 and 1 of port 0x61)
in al, 0x61
or al, 0x03
out 0x61, al

; Wait ~1 second
mov cx, 0xFFFF
.delay:
    nop
    loop .delay

; Disable speaker
in al, 0x61
and al, 0xFC
out 0x61, al

; Exit to DOS
mov ah, 0x4C
int 0x21
