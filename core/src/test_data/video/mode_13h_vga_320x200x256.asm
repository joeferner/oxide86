; VGA Graphics Mode 0x13 Test
; 320x200, 256 Colors, linear framebuffer at A000:0000
; Each byte is a DAC palette index; 320 bytes per row
; Displays 8 horizontal color bands using a custom DAC palette
; Tests: INT 10h mode set, DAC port I/O, direct linear framebuffer writes

[CPU 8086]
org 0x100

start:
    ; Switch to VGA mode 0x13 (320x200, 256 colors)
    mov ah, 0x00
    mov al, 0x13
    int 0x10

    ; Program 8 DAC palette entries via ports 0x3C8 (write index) and 0x3C9 (RGB data)
    ; Each component is 6-bit (0-63)
    mov dx, 0x3C8
    mov al, 0           ; Start at palette index 0
    out dx, al

    mov dx, 0x3C9
    ; Index 0: black (R=0, G=0, B=0)
    xor al, al
    out dx, al
    out dx, al
    out dx, al
    ; Index 1: red (R=63, G=0, B=0)
    mov al, 63
    out dx, al
    xor al, al
    out dx, al
    out dx, al
    ; Index 2: green (R=0, G=63, B=0)
    xor al, al
    out dx, al
    mov al, 63
    out dx, al
    xor al, al
    out dx, al
    ; Index 3: blue (R=0, G=0, B=63)
    xor al, al
    out dx, al
    out dx, al
    mov al, 63
    out dx, al
    ; Index 4: yellow (R=63, G=63, B=0)
    mov al, 63
    out dx, al
    out dx, al
    xor al, al
    out dx, al
    ; Index 5: cyan (R=0, G=63, B=63)
    xor al, al
    out dx, al
    mov al, 63
    out dx, al
    out dx, al
    ; Index 6: magenta (R=63, G=0, B=63)
    mov al, 63
    out dx, al
    xor al, al
    out dx, al
    mov al, 63
    out dx, al
    ; Index 7: white (R=63, G=63, B=63)
    mov al, 63
    out dx, al
    out dx, al
    out dx, al

    ; Set up video segment (A000:0000)
    mov ax, 0xA000
    mov es, ax
    xor di, di

    ; Fill screen with 8 horizontal bands, 25 rows each (25 * 8 = 200 rows)
    ; Band N uses palette index N
    mov bl, 0           ; palette index

.band_loop:
    mov cx, 25 * 320    ; 25 rows * 320 pixels = 8000 bytes per band
    mov al, bl
    rep stosb
    inc bl
    cmp bl, 8
    jb .band_loop

    ; Wait for keypress
    mov ah, 0x00
    int 0x16

    ; Return to text mode
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    ; Exit
    mov ah, 0x4C
    mov al, 0x00
    int 0x21
