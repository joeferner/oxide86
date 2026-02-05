; CGA Graphics Mode Test
; Switches to CGA mode 0x04 (320x200, 4 colors) and draws colored boxes
; Tests CGA graphics functionality

org 0x100

start:
    ; Switch to CGA graphics mode 0x04 (320x200, 4 colors)
    mov ah, 0x00        ; Set video mode
    mov al, 0x04        ; Mode 0x04 - CGA 320x200, 4 colors
    int 0x10

    ; Set CGA palette to palette 1 (cyan, magenta, white)
    ; Port 0x3D9 controls CGA color select register
    mov dx, 0x3D9
    mov al, 0x30        ; Bit 5=1 selects palette 1, bit 4=0 for black background
    out dx, al

    ; Set up video memory segment
    mov ax, 0xB800
    mov es, ax

    ; Draw colored boxes
    ; Each byte represents 4 pixels (2 bits per pixel)
    ; Pixel encoding: 00=background, 01=color1, 10=color2, 11=color3
    ; CGA uses interlaced memory: even lines at 0x0000-0x1F3F, odd at 0x2000-0x3F3F

    ; Box 1: Top-left, Cyan (color 1 = 01 pattern)
    mov cx, 40          ; 40 rows
    mov di, 0           ; Start at row 0 (even line, bank 0)
    mov bx, 0           ; Line counter
box1_loop:
    push cx
    mov cx, 10          ; 10 bytes = 40 pixels wide
    push di
box1_inner:
    mov byte [es:di], 0x55  ; 01010101 = 4 cyan pixels
    inc di
    loop box1_inner
    pop di
    ; Alternate between bank 0 (even) and bank 1 (odd)
    test bx, 1
    jz box1_even
    sub di, 0x2000 - 80  ; Odd to even: back to bank 0, next line
    jmp box1_next
box1_even:
    add di, 0x2000       ; Even to odd: jump to bank 1
box1_next:
    inc bx
    pop cx
    loop box1_loop

    ; Box 2: Top-middle, Magenta (color 2 = 10 pattern)
    mov cx, 40
    mov di, 20          ; Start at column 80 pixels (20 bytes)
    mov bx, 0
box2_loop:
    push cx
    mov cx, 10
    push di
box2_inner:
    mov byte [es:di], 0xAA  ; 10101010 = 4 magenta pixels
    inc di
    loop box2_inner
    pop di
    test bx, 1
    jz box2_even
    sub di, 0x2000 - 80
    jmp box2_next
box2_even:
    add di, 0x2000
box2_next:
    inc bx
    pop cx
    loop box2_loop

    ; Box 3: Top-right, White (color 3 = 11 pattern)
    mov cx, 40
    mov di, 40          ; Start at column 160 pixels (40 bytes)
    mov bx, 0
box3_loop:
    push cx
    mov cx, 10
    push di
box3_inner:
    mov byte [es:di], 0xFF  ; 11111111 = 4 white pixels
    inc di
    loop box3_inner
    pop di
    test bx, 1
    jz box3_even
    sub di, 0x2000 - 80
    jmp box3_next
box3_even:
    add di, 0x2000
box3_next:
    inc bx
    pop cx
    loop box3_loop

    ; Box 4: Bottom-left, Cyan (pattern)
    ; Row 100 is even, so in bank 0: (100/2) * 80 = 4000
    mov cx, 40
    mov di, 4000        ; Start at row 100 (even line, bank 0)
    mov bx, 0           ; Line counter
box4_loop:
    push cx
    mov cx, 10
    push di
box4_inner:
    mov byte [es:di], 0x55
    inc di
    loop box4_inner
    pop di
    ; Alternate between bank 0 (even) and bank 1 (odd)
    ; Bank 0 lines add 80, bank 1 lines add 80 + (0x2000 - 80) = 0x2000 - 80
    test bx, 1          ; Check if odd line
    jz box4_even
    ; Odd line: from bank 1 to bank 0, subtract (0x2000 - 80)
    sub di, 0x2000 - 80
    jmp box4_next
box4_even:
    ; Even line: from bank 0 to bank 1, add 0x2000
    add di, 0x2000
box4_next:
    inc bx
    pop cx
    loop box4_loop

    ; Box 5: Bottom-middle, Mixed pattern
    mov cx, 40
    mov di, 4020        ; Row 100, column 80 (even line, bank 0)
    mov bx, 0           ; Line counter
box5_loop:
    push cx
    mov cx, 10
    push di
box5_inner:
    mov byte [es:di], 0xE4  ; 11100100 = white, white, magenta, background
    inc di
    loop box5_inner
    pop di
    test bx, 1
    jz box5_even
    sub di, 0x2000 - 80
    jmp box5_next
box5_even:
    add di, 0x2000
box5_next:
    inc bx
    pop cx
    loop box5_loop

    ; Box 6: Bottom-right, All colors striped
    mov cx, 40
    mov di, 4040        ; Row 100, column 160 (even line, bank 0)
    mov bx, 0           ; Line counter
box6_loop:
    push cx
    mov cx, 10
    push di
box6_inner:
    ; Create a stripe pattern: bg, cyan, magenta, white
    mov byte [es:di], 0xE4  ; 11100100
    inc di
    loop box6_inner
    pop di
    test bx, 1
    jz box6_even
    sub di, 0x2000 - 80
    jmp box6_next
box6_even:
    add di, 0x2000
box6_next:
    inc bx
    pop cx
    loop box6_loop

    ; Display message at bottom
    ; Note: In graphics mode, we need to use teletype function
    mov ah, 0x13        ; Write string
    mov al, 0x01        ; Update cursor
    mov bh, 0x00        ; Page 0
    mov bl, 0x0F        ; White color
    mov cx, msg_len     ; String length
    mov dh, 24          ; Row 24 (bottom)
    mov dl, 0           ; Column 0
    push cs
    pop es
    mov bp, message
    int 0x10

    ; Wait for keypress
    mov ah, 0x00
    int 0x16

    ; Return to text mode
    mov ah, 0x00
    mov al, 0x03        ; Mode 0x03 - 80x25 text
    int 0x10

    ; Exit program
    mov ah, 0x4C
    int 0x21

message:
    db 'CGA Graphics Mode - Press any key to return to text mode'
msg_len equ $ - message
