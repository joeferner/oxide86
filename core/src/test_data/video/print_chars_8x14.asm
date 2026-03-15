; Prints all 256 ASCII characters (0x00-0xFF) in EGA mode 0x10 (640x350x16)
; using the 8x14 EGA font via BIOS INT 10h AH=09h (write char with attribute),
; which reads glyphs from the INT 43h font vector.
; Characters are laid out in a 16x16 grid (row = char/16, col = char%16).
; Waits for a keypress before exiting (assert_screen happens at the wait).

[CPU 8086]
org 0x100

start:
    ; Set EGA mode 0x10 (640x350, 16 colors, 8x14 font)
    mov ah, 0x00
    mov al, 0x10
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

    ; Write character with attribute 0x0F (white on black)
    mov al, [char_idx]
    mov ah, 0x09
    xor bh, bh
    mov bl, 0x0F
    mov cx, 1
    int 0x10

    inc byte [char_idx]
    jnz .next_char      ; loop until byte wraps 255->0

    ; Wait for keypress — assert_screen is taken here
    mov ah, 0x00
    int 0x16

    ; Exit with code 0
    mov ah, 0x4C
    xor al, al
    int 0x21

char_idx db 0
