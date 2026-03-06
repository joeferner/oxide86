; INT 10h Function 0Ah - Write Character Only at Cursor (no attribute change)
; Writes a character with 0x09 to set both char and attribute,
; then overwrites with 0x0A to change only the character (attribute preserved)

[CPU 8086]
org 0x0100

start:
    ; Position cursor at row 0, col 0
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 0
    int 0x10

    ; Write 'A' with attribute 0x1E (yellow on blue) using function 09h
    mov ah, 0x09
    mov al, 'A'
    mov bh, 0
    mov bl, 0x1E
    mov cx, 1
    int 0x10

    ; Overwrite with 'Z' using function 0Ah (char only, no attribute change)
    mov ah, 0x0A
    mov al, 'Z'
    mov bh, 0
    mov cx, 1
    int 0x10

    ; Read back: char should be 'Z', attribute should still be 0x1E
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'Z'
    jne fail
    cmp ah, 0x1E
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
