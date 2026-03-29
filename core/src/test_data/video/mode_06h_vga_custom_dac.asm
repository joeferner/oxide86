; Test: Mode 06h with custom VGA DAC palette programmed after mode set
;
; Replicates observed real-program behavior:
;   1. Set mode 06h (VGA resets DAC to defaults)
;   2. Program DAC register 16 = RGB(0,0,0)
;   3. Set all AC palette registers via INT 10h AH=10h AL=02h
;   4. Reprogram DAC registers 0-15 with custom values via INT 10h AH=10h AL=12h
;      - DAC[0]  = black   (background)
;      - DAC[1]  = white   (used via AC in text)
;      - DAC[15] = RGB(35,54,6) greenish  (mode 06h fg via cga_bg=15)
;
; Expected: foreground pixels appear as DAC[15] = greenish, background black.
; This verifies that custom DAC reprogramming after mode 06h is applied correctly.

[CPU 8086]
org 0x100

start:
    ; Set mode 06h (640x200 2-color CGA graphics)
    ; VGA resets DAC to defaults here (DAC[15] = white)
    mov ah, 0x00
    mov al, 0x06
    int 0x10

    ; --- Replicate real program's post-mode-set palette programming ---

    ; 1. Set DAC register 16 = RGB(0,0,0)
    mov ah, 0x10
    mov al, 0x10        ; Set individual DAC register
    mov bx, 16          ; Register 16
    mov dh, 0           ; Red = 0
    mov ch, 0           ; Green = 0
    mov cl, 0           ; Blue = 0
    int 0x10

    ; 2. Set all AC palette registers (17-byte table at DS:ac_table)
    mov ah, 0x10
    mov al, 0x02        ; Set all AC palette registers + border
    push ds
    pop es
    mov dx, ac_table
    int 0x10

    ; 3. Reprogram DAC registers 0-15 with custom colors (real program values)
    mov ah, 0x10
    mov al, 0x12        ; Set block of DAC registers
    mov bx, 0           ; Start at register 0
    mov cx, 16          ; 16 registers
    push ds
    pop es
    mov dx, dac_table
    int 0x10

    ; --- Draw content to show the custom fg color ---

    ; Draw a solid box using direct VRAM (0xB800:0000)
    ; Even scan lines: offset = (row/2)*80 + col_byte
    ; Odd  scan lines: 0x2000 + (row/2)*80 + col_byte
    mov ax, 0xB800
    mov es, ax

    ; Fill 16 pixel rows (char rows 0-1), cols 0..9 (80 pixels wide)
    mov cx, 8           ; 8 even rows (pixel rows 0,2,4,6,8,10,12,14)
    mov si, 0
.even_loop:
    mov di, si
    mov al, 0xFF
    mov [es:di+0], al
    mov [es:di+1], al
    mov [es:di+2], al
    mov [es:di+3], al
    mov [es:di+4], al
    mov [es:di+5], al
    mov [es:di+6], al
    mov [es:di+7], al
    mov [es:di+8], al
    mov [es:di+9], al
    add si, 80
    loop .even_loop

    mov cx, 8           ; 8 odd rows (pixel rows 1,3,5,7,9,11,13,15)
    mov si, 0x2000
.odd_loop:
    mov di, si
    mov al, 0xFF
    mov [es:di+0], al
    mov [es:di+1], al
    mov [es:di+2], al
    mov [es:di+3], al
    mov [es:di+4], al
    mov [es:di+5], al
    mov [es:di+6], al
    mov [es:di+7], al
    mov [es:di+8], al
    mov [es:di+9], al
    add si, 80
    loop .odd_loop

    ; Position cursor at row 5, col 10 and write text via INT 10h AH=09h
    ; attr=0x07 matches real program; in mode 06h glyph bits drive 1bpp pixels
    push ds
    pop es
    mov ah, 0x02
    mov bh, 0
    mov dh, 5
    mov dl, 10
    int 0x10

    mov si, msg
    call print_chars

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

; Print null-terminated string using AH=09h, advancing cursor manually
; attr=0x07 (fg=7), matching the real program
print_chars:
    push ax
    push bx
    push cx
    push dx
    push si
.loop:
    lodsb
    test al, al
    jz .done
    push ax
    mov ah, 0x03
    mov bh, 0
    int 0x10            ; get cursor -> DH=row, DL=col
    pop ax
    mov ah, 0x09
    mov bh, 0
    mov bl, 0x07        ; attr=0x07
    mov cx, 1
    int 0x10
    inc dl
    mov ah, 0x02
    mov bh, 0
    int 0x10            ; advance cursor
    jmp .loop
.done:
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; AC palette registers table (17 bytes: AC[0..15] + border)
; Identity mapping: AC[i] -> DAC[i]
ac_table:
    db 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07
    db 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F
    db 0x10                 ; border = DAC register 16

; DAC registers table (16 x 3 bytes, 6-bit RGB values)
; Exact values observed from the real program log
dac_table:
    db  0,  0,  0   ; DAC[0]  = black   (background)
    db 63, 63, 63   ; DAC[1]  = white
    db 56,  1,  1   ; DAC[2]  = dark red
    db 43,  3, 56   ; DAC[3]  = purple
    db  1,  0, 54   ; DAC[4]  = blue
    db  6,  4, 15   ; DAC[5]  = dark blue
    db 30, 52, 46   ; DAC[6]  = teal
    db 54,  6,  5   ; DAC[7]  = red
    db 15, 22, 52   ; DAC[8]  = blue-ish
    db 39, 10,  4   ; DAC[9]  = brown
    db  4,  4, 59   ; DAC[10] = bright blue
    db 14, 32, 57   ; DAC[11] = medium blue
    db 36,  0, 16   ; DAC[12] = dark magenta
    db 40, 22,  5   ; DAC[13] = orange-brown
    db 24, 35, 33   ; DAC[14] = teal-gray
    db 35, 54,  6   ; DAC[15] = greenish (mode 06h fg via cga_bg=15)

msg db "MODE 06H CUSTOM DAC (FG=DAC[15])", 0
