; Simple CGA Graphics Test - Single box at row 100
; Tests interlaced memory addressing

org 0x100

start:
    ; Switch to CGA mode 0x04
    mov ah, 0x00
    mov al, 0x04
    int 0x10

    ; Set palette
    mov dx, 0x3D9
    mov al, 0x30
    out dx, al

    ; Set up video segment
    mov ax, 0xB800
    mov es, ax

    ; Draw single box at row 100, column 0
    ; 40 rows, 20 bytes wide (80 pixels)
    mov cx, 40          ; 40 rows
    mov di, 4000        ; Row 100 (even line, bank 0)
    mov bx, 0           ; Line counter

draw_loop:
    push cx
    mov cx, 20          ; 20 bytes = 80 pixels wide
    push di
pixel_loop:
    mov byte [es:di], 0x55  ; Cyan pixels
    inc di
    loop pixel_loop
    pop di

    ; Alternate between banks
    test bx, 1
    jz even_line
    sub di, 0x2000 - 80
    jmp next_line
even_line:
    add di, 0x2000
next_line:
    inc bx
    pop cx
    loop draw_loop

    ; Wait for keypress
    mov ah, 0x00
    int 0x16

    ; Return to text mode
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    ; Exit
    mov ah, 0x4C
    int 0x21
