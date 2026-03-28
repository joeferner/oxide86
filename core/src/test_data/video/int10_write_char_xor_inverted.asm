; INT 10h AH=09h XorInverted cursor test
;
; Verifies that writing a character with ch bit 7 set in XOR mode (attr bit 7)
; uses glyph (ch & 0x7F) inverted — NOT the glyph for ch itself.
;
; ch=0x80 -> glyph for (0x80 & 0x7F) = 0x00 (blank, all zeros)
;         -> inverted: all ones -> full block XOR mask
;
; Step 1: Write 'Y' opaque, then apply XorInverted 0x80.
;         Screen should show inverted-Y (black Y on white background).
; Step 2: Apply XorInverted 0x80 again (self-inverse).
;         Screen should restore original 'Y' (white Y on black background).

[CPU 8086]
org 0x100

start:
    ; Switch to CGA mode 04h (320x200, 4-color)
    mov ah, 0x00
    mov al, 0x04
    int 0x10

    ; Select palette 1 (cyan/magenta/white), intensity on
    mov dx, 0x3D9
    mov al, 0x30
    out dx, al

    ; Hide the hardware cursor
    mov ah, 0x01
    mov cx, 0x2000
    int 0x10

    ; Position cursor at character cell (row=10, col=20)
    mov ah, 0x02
    mov bh, 0
    mov dh, 10
    mov dl, 20
    int 0x10

    ; Write 'Y' opaque with fg=3 (white)
    ; AL=char, BH=page, BL=attr (bit7=0 -> opaque, bits0-3=fg), CX=count
    mov ah, 0x09
    mov al, 'Y'
    mov bh, 0
    mov bl, 0x03        ; opaque, fg=3
    mov cx, 1
    int 0x10

    ; XorInverted: ch=0x80 (bit 7 = invert flag), attr=0x83 (XOR mode + fg=3)
    ; Expected: glyph fetched is 0x80 & 0x7F = 0x00 (all zeros), inverted to
    ;           all-ones, expanded to 2bpp full block, XOR'd with Y pixels.
    ;           Result: every pixel in the cell is toggled -> inverted Y.
    mov ah, 0x09
    mov al, 0x80
    mov bh, 0
    mov bl, 0x83        ; XOR mode (bit7) + fg=3
    mov cx, 1
    int 0x10

    ; Wait for keypress -> caller takes screenshot 1 (inverted Y)
    mov ah, 0x00
    int 0x16

    ; XorInverted again at same position -> self-inverse, restores Y
    mov ah, 0x02
    mov bh, 0
    mov dh, 10
    mov dl, 20
    int 0x10

    mov ah, 0x09
    mov al, 0x80
    mov bh, 0
    mov bl, 0x83
    mov cx, 1
    int 0x10

    ; Wait for keypress -> caller takes screenshot 2 (Y restored)
    mov ah, 0x00
    int 0x16

    ; Return to text mode and exit
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    mov ah, 0x4C
    mov al, 0x00
    int 0x21
