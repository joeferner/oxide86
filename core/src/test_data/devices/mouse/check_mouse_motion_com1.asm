; Mouse movement test: initialize COM1 as a serial mouse, read identification
; byte, then read one motion packet and verify dx=10, dy=5, no buttons.
;
; MS Mouse serial protocol (3-byte packets):
;   Byte 1: 0x40 | (LB<<5) | (RB<<4) | (Y7<<3) | (Y6<<2) | (X7<<1) | X6
;   Byte 2: X delta (lower 6 bits, 6-bit signed)
;   Byte 3: Y delta (lower 6 bits, 6-bit signed)
;
; For dx=10, dy=5, no buttons:
;   Byte 1 = 0x40  (sync bit only, no buttons, high bits = 0)
;   Byte 2 = 0x0A  (dx=10)
;   Byte 3 = 0x05  (dy=5)
;
; Initialization: 1200 baud (100), no parity (00), 1 stop bit (0), 7-bit (10)
;   AL = 1000_0010 = 0x82
; INT 14h AH=00h raises DTR, which triggers the 'M' identification byte.

[CPU 8086]
org 0x0100

start:
    ; Initialize COM1: 1200 baud, 7N1 (Microsoft Serial Mouse settings)
    ; This also raises DTR, causing the mouse to send its 'M' identification.
    mov ah, 0x00
    mov al, 0x82    ; 1200 baud, no parity, 1 stop bit, 7 data bits
    mov dx, 0x00    ; COM1
    int 0x14

    ; Read identification byte - must be 'M'
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80   ; bit 7 = timeout/error
    jnz fail
    cmp al, 'M'
    jne fail

    ; Read motion packet byte 1: 0x40 (sync, no buttons, zero high bits)
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 0x40
    jne fail

    ; Read motion packet byte 2: 0x0A (dx=10)
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 0x0A
    jne fail

    ; Read motion packet byte 3: 0x05 (dy=5)
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 0x05
    jne fail

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
