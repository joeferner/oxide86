; INT 10h Function 01h - Set Cursor Shape
; INT 10h Function 03h - Get Cursor Position (also returns cursor shape in CH/CL)
; Sets cursor start/end scan lines and verifies them via function 03h

[CPU 8086]
org 0x0100

start:
    ; Set cursor shape: start line = 6, end line = 7 (typical underline cursor)
    mov ah, 0x01
    mov ch, 6           ; cursor start scan line
    mov cl, 7           ; cursor end scan line
    int 0x10

    ; Get cursor position: returns shape in CH (start) and CL (end)
    mov ah, 0x03
    mov bh, 0
    int 0x10
    cmp ch, 6
    jne fail
    cmp cl, 7
    jne fail

    ; Set cursor shape: start line = 0, end line = 13 (block cursor)
    mov ah, 0x01
    mov ch, 0
    mov cl, 13
    int 0x10

    mov ah, 0x03
    mov bh, 0
    int 0x10
    cmp ch, 0
    jne fail
    cmp cl, 13
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
