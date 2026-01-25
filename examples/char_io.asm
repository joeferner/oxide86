; Character I/O using INT 21h
; Demonstrates reading and writing individual characters

BITS 16
ORG 0x0100

start:
    ; Print prompt
    mov dx, prompt
    mov ah, 0x09        ; Write string
    int 0x21

    ; Read a character with echo (AH=01h)
    mov ah, 0x01        ; Function 01h - Read character with echo
    int 0x21            ; Character is returned in AL

    ; Store the character
    mov bl, al          ; Save character in BL

    ; Print newline
    mov dl, 0x0A        ; Line feed
    mov ah, 0x02        ; Function 02h - Write character
    int 0x21

    ; Print "You typed: "
    mov dx, typed_msg
    mov ah, 0x09        ; Write string
    int 0x21

    ; Print the character we read
    mov dl, bl          ; Restore character
    mov ah, 0x02        ; Function 02h - Write character
    int 0x21

    ; Print newline
    mov dl, 0x0A
    mov ah, 0x02
    int 0x21

    ; Exit
    mov ah, 0x4C
    mov al, 0
    int 0x21

prompt:
    db 'Enter a character: $'

typed_msg:
    db 'You typed: $'
