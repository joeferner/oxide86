; INT 10h - CGA underline cursor shape (start=6, end=7)
; Verifies that a CGA-style cursor (scan lines 6-7 of 8) renders at the
; bottom of a 16-scanline VGA character cell, not the middle.

[CPU 8086]
org 0x0100

start:
    ; Write "CURSOR" at top-left so cursor position is visible
    mov ah, 0x02
    mov bh, 0
    mov dh, 0           ; row 0
    mov dl, 0           ; col 0
    int 0x10

    mov si, msg
.write_loop:
    lodsb
    or al, al
    jz .done_write
    mov ah, 0x0E
    int 0x10
    jmp .write_loop
.done_write:

    ; Move cursor back to col 0, row 0
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 0
    int 0x10

    ; Set CGA underline cursor shape (6-7) - typical DOS cursor
    mov ah, 0x01
    mov ch, 6           ; cursor start scan line (CGA style, out of 8)
    mov cl, 7           ; cursor end scan line
    int 0x10

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

msg db "CURSOR", 0
