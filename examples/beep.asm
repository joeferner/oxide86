; beep.asm - Test PC speaker with PIT Channel 2
; Compile: nasm -f bin beep.asm -o beep.com
; Run: cargo run -p emu86-native -- examples/beep.com

org 0x100

; Configure PIT Channel 2: Mode 3 (square wave), LSB+MSB, Binary
mov al, 0xB6        ; 10 11 011 0 = Channel 2, LSB+MSB, Mode 3, Binary
out 0x43, al

; Load count: 1193 (1193182 / 1193 ≈ 1000 Hz)
mov al, 0xA9        ; LSB of 1193 (0x04A9)
out 0x42, al
mov al, 0x04        ; MSB of 1193
out 0x42, al

; Enable speaker (set bits 0 and 1 of port 0x61)
in al, 0x61
or al, 0x03
out 0x61, al

; Wait approximately 1 second
mov cx, 0xFFFF
.delay:
    loop .delay

; Disable speaker (clear bits 0 and 1)
in al, 0x61
and al, 0xFC
out 0x61, al

; Exit
mov ah, 0x4C
int 0x21
