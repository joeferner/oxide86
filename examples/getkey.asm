; Simple keyboard input example
; Reads a single key and displays it
;
; Build and run:
;   ./examples/run.sh getkey.asm

org 0x0100                ; COM file format

section .text
start:
    ; Display prompt
    mov dx, prompt
    mov ah, 0x09
    int 0x21

    ; Wait for and read a keystroke (INT 16h, AH=00h)
    mov ah, 0x00
    int 0x16              ; Returns scan code in AH, ASCII in AL

    ; Save the key
    push ax

    ; Display "You pressed: "
    mov dx, msg
    mov ah, 0x09
    int 0x21

    ; Display the character
    pop ax                ; Restore the key
    mov dl, al
    mov ah, 0x02
    int 0x21

    ; Display newline
    mov dx, newline
    mov ah, 0x09
    int 0x21

    ; Exit program
    mov ah, 0x4C
    xor al, al
    int 0x21

section .data
prompt:     db 'Press any key...', 13, 10, '$'
msg:        db 'You pressed: $'
newline:    db 13, 10, '$'
