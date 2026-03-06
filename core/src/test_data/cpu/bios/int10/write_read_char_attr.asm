; INT 10h Function 09h - Write Character and Attribute at Cursor
; INT 10h Function 08h - Read Character and Attribute at Cursor
; Writes a character with a specific attribute and reads it back

[CPU 8086]
org 0x0100

start:
    ; Position cursor at row 0, col 0
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 0
    int 0x10

    ; Write 'A' (0x41) with attribute 0x07 (white on black), 1 time
    mov ah, 0x09
    mov al, 'A'
    mov bh, 0           ; page 0
    mov bl, 0x07        ; attribute: white text on black background
    mov cx, 1           ; repeat count
    int 0x10

    ; Cursor does NOT advance after 0x09 - still at 0,0
    mov ah, 0x08
    mov bh, 0           ; page 0
    int 0x10
    ; AL = character, AH = attribute
    cmp al, 'A'
    jne fail
    cmp ah, 0x07
    jne fail

    ; Write 'B' with bright attribute 0x0F (bright white on black), 3 times
    mov ah, 0x02
    mov bh, 0
    mov dh, 1
    mov dl, 5
    int 0x10

    mov ah, 0x09
    mov al, 'B'
    mov bh, 0
    mov bl, 0x0F
    mov cx, 3
    int 0x10

    ; Read back at row 1, col 5 - first written 'B'
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'B'
    jne fail
    cmp ah, 0x0F
    jne fail

    ; Move to col 7 (5+2) - third 'B'
    mov ah, 0x02
    mov bh, 0
    mov dh, 1
    mov dl, 7
    int 0x10

    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'B'
    jne fail
    cmp ah, 0x0F
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
