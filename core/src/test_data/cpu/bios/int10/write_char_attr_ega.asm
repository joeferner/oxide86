; INT 10h Function 09h - Write Character and Attribute in EGA mode 0x0D
; Sets EGA mode 0x0D (320x200 16-colour), writes several characters with different
; colours using AH=0x09 (which uses the ROM font at F000:C000 via INT 43h),
; waits for a key for visual inspection, then verifies VRAM is non-zero.

[CPU 8086]
org 0x0100

start:
    ; Set EGA mode 0x0D (320x200 16-colour)
    mov ah, 0x00
    mov al, 0x0D
    int 0x10

    ; Cursor to row 4, col 5
    mov ah, 0x02
    xor bx, bx
    mov dh, 4
    mov dl, 5
    int 0x10

    ; Write 'H' in yellow (colour 0x0E)
    mov ah, 0x09
    mov al, 'H'
    xor bh, bh
    mov bl, 0x0E
    mov cx, 1
    int 0x10

    ; Cursor to row 4, col 6
    mov ah, 0x02
    xor bx, bx
    mov dh, 4
    mov dl, 6
    int 0x10

    ; Write 'i' in cyan (colour 0x0B)
    mov ah, 0x09
    mov al, 'i'
    xor bh, bh
    mov bl, 0x0B
    mov cx, 1
    int 0x10

    ; Cursor to row 4, col 7
    mov ah, 0x02
    xor bx, bx
    mov dh, 4
    mov dl, 7
    int 0x10

    ; Write '!' in white (colour 0x0F)
    mov ah, 0x09
    mov al, '!'
    xor bh, bh
    mov bl, 0x0F
    mov cx, 1
    int 0x10

    ; Wait for key – visual assertion happens here
    mov ah, 0x00
    int 0x16

    ; Verify: VRAM for 'H' (colour 0x0E = binary 1110) at row 4, col 5, pixel row 0
    ;   pixel_y = 4*8 + 0 = 32   offset = 32*40 + 5 = 1285 = 0x0505
    ; Colour 0x0E: bit 0 = 0 (plane 0 empty), bit 1 = 1 (plane 1 has glyph).
    ; Select plane 1 via GC Read Map Select register before reading.
    mov dx, 0x3CE
    mov al, 0x04            ; GC register index 4 = Read Map Select
    out dx, al
    mov dx, 0x3CF
    mov al, 0x01            ; plane 1
    out dx, al

    mov ax, 0xA000
    mov es, ax
    mov al, [es:0x0505]
    cmp al, 0x00
    je fail

    ; Restore text mode and exit
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    mov ah, 0x4C
    mov al, 0x01
    int 0x21
