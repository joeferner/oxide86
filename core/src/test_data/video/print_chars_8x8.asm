; Prints all 256 ASCII characters (0x00-0xFF) using the 8x8 CGA font
; CGA graphics mode 04h (320x200 4-color), laid out in a 16x16 grid
; (row = char/16, col = char%16 — 40 cols available, 25 rows available)
; Waits for a keypress before exiting (assert_screen happens at the wait)

[CPU 8086]
org 0x100

start:
    ; Set CGA mode 04h (320x200, 4-color, 8x8 graphics font)
    mov ah, 0x00
    mov al, 0x04
    int 0x10

    mov [char_idx], byte 0

.next_char:
    ; Compute row = char_idx / 16 (AL), col = char_idx % 16 (AH)
    mov al, [char_idx]
    xor ah, ah
    mov bl, 16
    div bl              ; AL = row, AH = col

    ; Set cursor position: DH=row, DL=col, BH=page 0
    mov dh, al
    mov dl, ah
    xor bx, bx
    mov ah, 0x02
    int 0x10

    ; Write character with color 0x03 (white in CGA palette)
    mov al, [char_idx]
    mov ah, 0x09
    xor bh, bh
    mov bl, 0x03
    mov cx, 1
    int 0x10

    inc byte [char_idx]
    jnz .next_char      ; loop until byte wraps 255->0

    ; Wait for keypress — assert_screen is taken here
    mov ah, 0x00
    int 0x16

    ; Restore text mode and exit
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    mov ah, 0x4C
    xor al, al
    int 0x21

char_idx db 0
