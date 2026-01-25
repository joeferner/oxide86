; INT 10h Video Services Test Program
; Tests all implemented video BIOS functions
org 0x0100

start:
    ; Test AH=00h - Set video mode
    mov ah, 0x00
    mov al, 0x03        ; 80x25 color text mode
    int 0x10

    ; Test AH=06h - Scroll up (clear screen)
    mov ah, 0x06
    mov al, 0           ; Clear entire window
    mov bh, 0x07        ; White on black
    mov cx, 0           ; Top-left (0,0)
    mov dx, 0x184F      ; Bottom-right (24,79)
    int 0x10

    ; Test AH=02h - Set cursor position (row 2, col 5)
    mov ah, 0x02
    mov bh, 0           ; Page 0
    mov dh, 2           ; Row 2
    mov dl, 5           ; Column 5
    int 0x10

    ; Test AH=09h - Write character with attribute
    mov ah, 0x09
    mov al, 'X'         ; Character
    mov bl, 0x1F        ; White on blue
    mov cx, 10          ; Write 10 times
    int 0x10

    ; Test AH=02h - Set cursor position (row 4, col 0)
    mov ah, 0x02
    mov bh, 0
    mov dh, 4
    mov dl, 0
    int 0x10

    ; Test AH=0Eh - Teletype output
    mov ah, 0x0E
    mov al, 'H'
    int 0x10
    mov al, 'e'
    int 0x10
    mov al, 'l'
    int 0x10
    mov al, 'l'
    int 0x10
    mov al, 'o'
    int 0x10
    mov al, ','
    int 0x10
    mov al, ' '
    int 0x10
    mov al, 'V'
    int 0x10
    mov al, 'i'
    int 0x10
    mov al, 'd'
    int 0x10
    mov al, 'e'
    int 0x10
    mov al, 'o'
    int 0x10
    mov al, '!'
    int 0x10

    ; Test CR/LF
    mov al, 0x0D        ; Carriage return
    int 0x10
    mov al, 0x0A        ; Line feed
    int 0x10

    ; Test AH=13h - Write string (centered message)
    mov ah, 0x13
    mov al, 0x01        ; Update cursor, no attributes in string
    mov bh, 0           ; Page 0
    mov bl, 0x0E        ; Yellow on black
    mov cx, 26          ; Length
    mov dh, 12          ; Row 12 (center)
    mov dl, 27          ; Column 27 (approximately centered)
    mov bp, message
    push cs
    pop es
    int 0x10

    ; Test teletype with newlines (move to row 14)
    mov ah, 0x02
    mov bh, 0
    mov dh, 14
    mov dl, 0
    int 0x10

    mov ah, 0x0E
    mov al, 'L'
    int 0x10
    mov al, 'i'
    int 0x10
    mov al, 'n'
    int 0x10
    mov al, 'e'
    int 0x10
    mov al, ' '
    int 0x10
    mov al, '1'
    int 0x10
    mov al, 0x0D
    int 0x10
    mov al, 0x0A
    int 0x10
    mov al, 'L'
    int 0x10
    mov al, 'i'
    int 0x10
    mov al, 'n'
    int 0x10
    mov al, 'e'
    int 0x10
    mov al, ' '
    int 0x10
    mov al, '2'
    int 0x10

    ; Test backspace
    mov al, 0x08        ; Backspace
    int 0x10
    mov al, 0x08
    int 0x10
    mov al, '3'         ; Should replace '2'
    int 0x10

    ; Test AH=06h - Scroll up a region (rows 20-23, cols 10-70)
    mov ah, 0x06
    mov al, 1           ; Scroll 1 line
    mov bh, 0x17        ; White on blue
    mov ch, 20          ; Top row
    mov cl, 10          ; Left column
    mov dh, 23          ; Bottom row
    mov dl, 70          ; Right column
    int 0x10

    ; Test AH=07h - Scroll down a region (rows 20-23, cols 10-70)
    mov ah, 0x07
    mov al, 1           ; Scroll 1 line
    mov bh, 0x2F        ; White on green
    mov ch, 20          ; Top row
    mov cl, 10          ; Left column
    mov dh, 23          ; Bottom row
    mov dl, 70          ; Right column
    int 0x10

    ; Position cursor at bottom for final message
    mov ah, 0x02
    mov bh, 0
    mov dh, 24          ; Bottom row
    mov dl, 0
    int 0x10

    ; Final message
    mov ah, 0x13
    mov al, 0x01
    mov bh, 0
    mov bl, 0x0A        ; Green on black
    mov cx, 24
    mov dh, 24
    mov dl, 28
    mov bp, final_msg
    push cs
    pop es
    int 0x10

    ; Halt
    hlt

message: db 'INT 10h Video Test Success'
final_msg: db 'Press Ctrl+C to exit...'
