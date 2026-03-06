; INT 10h Function 02h - Set Cursor Position
; INT 10h Function 03h - Get Cursor Position
; Sets cursor to various positions and reads them back to verify

[CPU 8086]
org 0x0100

start:
    ; Set cursor to row 5, col 10 on page 0
    mov ah, 0x02
    mov bh, 0           ; page 0
    mov dh, 5           ; row
    mov dl, 10          ; col
    int 0x10

    ; Get cursor position for page 0
    mov ah, 0x03
    mov bh, 0
    int 0x10
    ; DH = row, DL = col
    cmp dh, 5
    jne fail
    cmp dl, 10
    jne fail

    ; Set cursor to row 24, col 79 (bottom-right of 80x25 screen)
    mov ah, 0x02
    mov bh, 0
    mov dh, 24
    mov dl, 79
    int 0x10

    mov ah, 0x03
    mov bh, 0
    int 0x10
    cmp dh, 24
    jne fail
    cmp dl, 79
    jne fail

    ; Set cursor to row 0, col 0 (top-left)
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 0
    int 0x10

    mov ah, 0x03
    mov bh, 0
    int 0x10
    cmp dh, 0
    jne fail
    cmp dl, 0
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
