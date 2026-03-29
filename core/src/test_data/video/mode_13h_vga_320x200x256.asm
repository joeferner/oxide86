; VGA Graphics Mode 0x13 Test
; 320x200, 256 Colors, linear framebuffer at A000:0000
; Each byte is a DAC palette index; 320 bytes per row
; Displays 8 horizontal color bands with labels using INT 10h AH=0Eh
; Tests: INT 10h mode set, DAC port I/O, direct linear framebuffer writes,
;        AH=0Eh teletype in VGA graphics mode (draw_char_cga_graphics mode 13h)

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

    ; --- Label each color band using AH=0Eh (teletype in VGA graphics mode) ---
    ; Mode 0x13 text grid: 40 cols x 25 rows (8x8 pixels per cell)
    ; Each band spans 25 pixel rows = ~3 char rows; label at band center row
    ; Opaque draw: set pixels = fg_color (BL), unset pixels = palette 0 (black)
    ;
    ; Band pixel rows -> center char row:
    ;   Band 0 (  0-24): row  1      Band 4 (100-124): row 13
    ;   Band 1 ( 25-49): row  4      Band 5 (125-149): row 16
    ;   Band 2 ( 50-74): row  7      Band 6 (150-174): row 19
    ;   Band 3 ( 75-99): row 10      Band 7 (175-199): row 22

    ; Band 0: Black - white text (7)
    mov ah, 0x02
    mov bh, 0
    mov dh, 1
    mov dl, 16
    int 0x10
    mov si, msg_black
    mov bl, 7
    call print_string

    ; Band 1: Red - white text (7)
    mov ah, 0x02
    mov bh, 0
    mov dh, 4
    mov dl, 16
    int 0x10
    mov si, msg_red
    mov bl, 7
    call print_string

    ; Band 2: Green - white text (7)
    mov ah, 0x02
    mov bh, 0
    mov dh, 7
    mov dl, 16
    int 0x10
    mov si, msg_green
    mov bl, 7
    call print_string

    ; Band 3: Blue - white text (7)
    mov ah, 0x02
    mov bh, 0
    mov dh, 10
    mov dl, 16
    int 0x10
    mov si, msg_blue
    mov bl, 7
    call print_string

    ; Band 4: Yellow - blue text (3)
    mov ah, 0x02
    mov bh, 0
    mov dh, 13
    mov dl, 16
    int 0x10
    mov si, msg_yellow
    mov bl, 3
    call print_string

    ; Band 5: Cyan - red text (1)
    mov ah, 0x02
    mov bh, 0
    mov dh, 16
    mov dl, 16
    int 0x10
    mov si, msg_cyan
    mov bl, 1
    call print_string

    ; Band 6: Magenta - white text (7)
    mov ah, 0x02
    mov bh, 0
    mov dh, 19
    mov dl, 16
    int 0x10
    mov si, msg_magenta
    mov bl, 7
    call print_string

    ; Band 7: White - blue text (3)
    mov ah, 0x02
    mov bh, 0
    mov dh, 22
    mov dl, 16
    int 0x10
    mov si, msg_white
    mov bl, 3
    call print_string

    ; Header: "VGA Mode 0x13" on band 0 (black), col 7, white text
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 7
    int 0x10
    mov si, msg_header
    mov bl, 7
    call print_string

    ; Info line: "320x200, 256 Colors" on band 0 (black), col 10, white text
    mov ah, 0x02
    mov bh, 0
    mov dh, 2
    mov dl, 10
    int 0x10
    mov si, msg_info
    mov bl, 7
    call print_string

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

; Print null-terminated string using BIOS teletype (AH=0Eh, opaque in graphics mode)
; Input: SI = pointer to string, BL = foreground color (palette index for mode 13h)
print_string:
    push ax
    push bx
    push cx
    push dx
    push si
    push di
.loop:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0E
    mov bh, 0
    int 0x10
    jmp .loop
.done:
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Data
msg_header  db "VGA Mode 0x13", 0
msg_info    db "320x200, 256 Colors", 0
msg_black   db "0:Black", 0
msg_red     db "1:Red", 0
msg_green   db "2:Green", 0
msg_blue    db "3:Blue", 0
msg_yellow  db "4:Yellow", 0
msg_cyan    db "5:Cyan", 0
msg_magenta db "6:Magenta", 0
msg_white   db "7:White", 0
