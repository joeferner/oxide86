; INT 10h Function 0Eh - Teletype Output
; Writes "Hello World!" to the screen character by character
; Verifies cursor advances with each character written

[CPU 8086]
org 0x0100

start:
    ; Set video mode 3 (80x25 color text) to start clean
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    ; Get initial cursor position - should be 0,0 after mode set
    mov ah, 0x03
    mov bh, 0
    int 0x10
    cmp dh, 0
    jne fail
    cmp dl, 0
    jne fail

    ; Write "Hello!" via teletype (AH=0Eh)
    mov bx, 0           ; BH=page 0, BL=foreground (text mode ignores BL)
    mov si, msg
.write_loop:
    mov al, [si]
    cmp al, 0
    je  done_writing
    mov ah, 0x0E
    int 0x10
    inc si
    jmp .write_loop

done_writing:
    ; Cursor should now be at col 6 (length of "Hello!"), row 0
    mov ah, 0x03
    mov bh, 0
    int 0x10
    cmp dh, 0
    jne fail
    cmp dl, 6
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

msg db 'Hello!', 0
