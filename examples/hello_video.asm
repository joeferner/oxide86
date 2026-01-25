; hello_video.asm - Write "Hello, World!" directly to VGA video memory
; VGA text mode memory starts at 0xB8000
; Each character cell is 2 bytes: [character, attribute]
; Attribute: bits 0-3 foreground color, bits 4-6 background color, bit 7 blink

org 0x0100

start:
    ; Set DS to video memory segment (0xB800)
    ; We use DS instead of ES to avoid segment override prefixes
    mov ax, 0xB800
    mov ds, ax

    ; BX will be our pointer into video memory
    mov bx, 0

    ; Write "Hello, World!" with white on blue background
    ; Attribute: 0x1F = white (15) on blue (1)

    ; 'H'
    mov byte [bx], 'H'
    mov byte [bx+1], 0x1F
    add bx, 2

    ; 'e'
    mov byte [bx], 'e'
    mov byte [bx+1], 0x1F
    add bx, 2

    ; 'l'
    mov byte [bx], 'l'
    mov byte [bx+1], 0x1F
    add bx, 2

    ; 'l'
    mov byte [bx], 'l'
    mov byte [bx+1], 0x1F
    add bx, 2

    ; 'o'
    mov byte [bx], 'o'
    mov byte [bx+1], 0x1F
    add bx, 2

    ; ','
    mov byte [bx], ','
    mov byte [bx+1], 0x1F
    add bx, 2

    ; ' '
    mov byte [bx], ' '
    mov byte [bx+1], 0x1F
    add bx, 2

    ; 'W'
    mov byte [bx], 'W'
    mov byte [bx+1], 0x1F
    add bx, 2

    ; 'o'
    mov byte [bx], 'o'
    mov byte [bx+1], 0x1F
    add bx, 2

    ; 'r'
    mov byte [bx], 'r'
    mov byte [bx+1], 0x1F
    add bx, 2

    ; 'l'
    mov byte [bx], 'l'
    mov byte [bx+1], 0x1F
    add bx, 2

    ; 'd'
    mov byte [bx], 'd'
    mov byte [bx+1], 0x1F
    add bx, 2

    ; '!'
    mov byte [bx], '!'
    mov byte [bx+1], 0x1F

    ; Write some colored text on the second row
    ; Row 1 starts at offset 160 (80 columns * 2 bytes)
    mov bx, 160

    ; Write "RGB" with different colors
    mov byte [bx], 'R'
    mov byte [bx+1], 0x4F  ; Red on white
    add bx, 2

    mov byte [bx], 'G'
    mov byte [bx+1], 0x2F  ; Green on white
    add bx, 2

    mov byte [bx], 'B'
    mov byte [bx+1], 0x1F  ; Blue on white
    add bx, 2

    mov byte [bx], 'Y'
    mov byte [bx+1], 0xEF  ; Yellow on white
    add bx, 2

    ; Halt the CPU
    hlt
