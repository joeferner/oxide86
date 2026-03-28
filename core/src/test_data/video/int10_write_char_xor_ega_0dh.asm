; INT 10h AH=09h XOR cursor blink in EGA mode 0Dh
;
; Verifies that in EGA mode 0x0D, ch=0xDB (full block, all pixels on) in XOR
; mode (BL bit 7) correctly XORs all pixels in the character cell — i.e. bit 7
; of AL is NOT an invert flag in EGA modes, so 0xDB is used as glyph 0xDB
; (all-ones), not as glyph 0x5B ('[') inverted.
;
; Step 1: Write 'Y' opaque, then apply XOR cursor (ch=0xDB, BL bit 7 set).
;         Screen should show the cell inverted (full-block XOR over Y).
; Step 2: Apply XOR cursor again (self-inverse) -> Y is restored.

[CPU 8086]
org 0x100

start:
    ; Switch to EGA mode 0x0D (320x200, 16 colors)
    mov ah, 0x00
    mov al, 0x0D
    int 0x10

    ; Restore map mask to all planes
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    mov dx, 0x3C5
    mov al, 0x0F
    out dx, al

    ; Hide the hardware cursor
    mov ah, 0x01
    mov cx, 0x2000
    int 0x10

    ; Position cursor at character cell (row=12, col=20)
    mov ah, 0x02
    mov bh, 0
    mov dh, 12
    mov dl, 20
    int 0x10

    ; Write 'Y' opaque with fg=15 (white, all planes)
    mov ah, 0x09
    mov al, 'Y'
    mov bh, 0
    mov bl, 0x0F        ; opaque, fg=15
    mov cx, 1
    int 0x10

    ; XOR cursor: ch=0xDB (full block, all-ones glyph), BL bit 7 = XOR mode
    ; In EGA, bit 7 of AL is NOT an invert flag: glyph 0xDB is all pixels on.
    ; XOR all-ones with Y pixels -> inverts entire cell.
    mov ah, 0x09
    mov al, 0xDB
    mov bh, 0
    mov bl, 0x8F        ; XOR mode (bit7) + fg=15
    mov cx, 1
    int 0x10

    ; Wait for keypress -> caller takes screenshot 1 (cursor over Y, inverted cell)
    mov ah, 0x00
    int 0x16

    ; Reposition cursor and apply XOR again -> self-inverse, restores Y
    mov ah, 0x02
    mov bh, 0
    mov dh, 12
    mov dl, 20
    int 0x10

    mov ah, 0x09
    mov al, 0xDB
    mov bh, 0
    mov bl, 0x8F
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
