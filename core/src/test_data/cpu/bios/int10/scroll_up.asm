; INT 10h Function 06h - Scroll Up
; Writes text to two rows, scrolls up by 1, verifies content moved

[CPU 8086]
org 0x0100

start:
    ; Set video mode 3 to start with clean screen
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    ; Write 'X' at row 1, col 0 with attribute 0x07
    mov ah, 0x02
    mov bh, 0
    mov dh, 1
    mov dl, 0
    int 0x10

    mov ah, 0x09
    mov al, 'X'
    mov bh, 0
    mov bl, 0x07
    mov cx, 1
    int 0x10

    ; Verify 'X' is at row 1, col 0 before scroll
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'X'
    jne fail

    ; Scroll up 1 line across full screen (AL=1, BH=attr for blank, CH/CL=top-left, DH/DL=bottom-right)
    mov ah, 0x06
    mov al, 1           ; scroll 1 line
    mov bh, 0x07        ; blank line attribute
    mov ch, 0           ; top row
    mov cl, 0           ; left col
    mov dh, 24          ; bottom row
    mov dl, 79          ; right col
    int 0x10

    ; After scrolling up 1 line, 'X' from row 1 should now be at row 0, col 0
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 0
    int 0x10

    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'X'
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
