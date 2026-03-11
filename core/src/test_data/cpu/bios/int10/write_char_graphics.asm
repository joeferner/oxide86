; INT 10h Function 09h - Write Character and Attribute at Cursor (Graphics Mode)
; Sets CGA mode 04h (320x200x4), writes characters, verifies VRAM, then waits
; for a key so the graphics output can be captured as a visual regression.

[CPU 8086]
org 0x0100

start:
    ; Set video mode 04h (CGA 320x200 4-color)
    mov ah, 0x00
    mov al, 0x04
    int 0x10

    ; Position cursor at row 0, col 0
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 0
    int 0x10

    ; Write 'A' with color 0x03 (white), opaque mode
    mov ah, 0x09
    mov al, 'A'
    mov bh, 0
    mov bl, 0x03        ; color 3 = brown, bit 7 clear = opaque
    mov cx, 1
    int 0x10

    ; Write 'B' at col 1 with color 0x02 (magenta), opaque mode
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 1
    int 0x10

    mov ah, 0x09
    mov al, 'B'
    mov bh, 0
    mov bl, 0x02        ; color 2 = red, opaque
    mov cx, 1
    int 0x10

    ; Write 'C' at col 2 with XOR mode (BL bit 7 set)
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 2
    int 0x10

    mov ah, 0x09
    mov al, 'C'
    mov bh, 0
    mov bl, 0x83        ; color 3 = brown, bit 7 set = XOR mode
    mov cx, 1
    int 0x10

    ; Wait for key press - test asserts screen in graphics mode here
    mov ah, 0x00
    int 0x16

    ; Verify VRAM: 0xB800:0000 should be non-zero ('A' pixels at row 0, even bank)
    mov ax, 0xB800
    mov es, ax
    mov al, es:[0x0000]
    cmp al, 0x00
    je fail

    ; 0xB800:0002 should be non-zero ('B' pixels, col 1 = byte offset 2 at 2bpp)
    mov al, es:[0x0002]
    cmp al, 0x00
    je fail

    ; 0xB800:0004 should be non-zero ('C' XOR pixels)
    mov al, es:[0x0004]
    cmp al, 0x00
    je fail

    ; Restore text mode
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
