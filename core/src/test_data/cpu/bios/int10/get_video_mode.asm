; INT 10h Function 0Fh - Get Current Video Mode
; After BIOS init, default mode should be 3 (80x25 color text)
; AL = mode number, AH = number of columns, BH = active display page

[CPU 8086]
org 0x0100

start:
    ; Query current video mode
    mov ah, 0x0F
    int 0x10

    ; AL = video mode (should be 3 - 80x25 color text)
    cmp al, 0x03
    jne fail

    ; AH = number of columns (should be 80)
    cmp ah, 80
    jne fail

    ; BH = active display page (should be 0)
    cmp bh, 0
    jne fail

    ; Now switch to mode 2 (80x25 grayscale text) and verify
    mov ah, 0x00
    mov al, 0x02
    int 0x10

    mov ah, 0x0F
    int 0x10

    cmp al, 0x02
    jne fail
    cmp ah, 80
    jne fail
    cmp bh, 0
    jne fail

    ; Switch back to mode 3
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    mov ah, 0x0F
    int 0x10
    cmp al, 0x03
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
