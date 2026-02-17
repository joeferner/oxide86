; VGA Graphics Mode 0x13 Test
; 320x200, 256 Colors (linear framebuffer at A000:0000)
; 1 byte per pixel, offset = y * 320 + x
; Displays all 256 colors as a gradient grid and labeled palette strips

[CPU 8086]
org 0x100

start:
    ; Switch to VGA mode 0x13 (320x200, 256 colors)
    mov ah, 0x00
    mov al, 0x13
    int 0x10

    ; Set up video segment for direct memory access
    mov ax, 0xA000
    mov es, ax

    ; --- Draw 256-color gradient: 16 rows x 16 columns of color blocks ---
    ; Each block is 20 pixels wide and 12 pixels tall
    ; Block (c, r) uses color index c + r*16

    xor bx, bx          ; bx = color index (0..255)
.block_outer:
    ; Calculate row and column of this block
    mov ax, bx
    and al, 0x0F        ; column = color & 0x0F
    mov [block_col], al
    mov ax, bx
    mov cl, 4
    shr ax, cl           ; row = color >> 4
    mov [block_row], al

    ; Calculate pixel start: start_y = block_row * 12, start_x = block_col * 20
    xor ax, ax
    mov al, [block_row]
    mov cx, 12
    mul cx              ; AX = block_row * 12
    mov [start_y], ax

    xor ax, ax
    mov al, [block_col]
    mov cx, 20
    mul cx              ; AX = block_col * 20
    mov [start_x], ax

    ; Draw 12 rows of 20 pixels each; DX = current pixel row (start_y)
    mov dx, [start_y]
    mov cx, 12          ; row count
.draw_row:
    ; offset = row * 320 + col_start
    push cx             ; save outer loop counter
    push dx             ; save row (mul destroys DX)
    mov ax, dx          ; ax = current pixel row
    mov cx, 320
    mul cx              ; AX = row * 320 (fits in 16 bits for rows 0-199)
    add ax, [start_x]
    mov di, ax
    pop dx              ; restore row

    ; Draw 20 pixels
    mov cx, 20
    mov al, bl          ; color index
.draw_pixel:
    stosb
    loop .draw_pixel

    inc dx              ; next row
    pop cx              ; restore outer loop counter
    loop .draw_row

    inc bx
    cmp bx, 256
    jb .block_outer

    ; --- Draw some text labels using INT 10h AH=0Eh (teletype) ---

    ; Title at character row 22
    mov ah, 0x02
    mov bh, 0
    mov dh, 22
    mov dl, 5
    int 0x10

    mov si, msg_title
    mov bl, 15          ; White text
    call print_string

    ; Info line at character row 23
    mov ah, 0x02
    mov bh, 0
    mov dh, 23
    mov dl, 3
    int 0x10

    mov si, msg_info
    mov bl, 14          ; Yellow text
    call print_string

    ; Wait for keypress
    mov ah, 0x00
    int 0x16

    ; Return to text mode 0x03
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    ; Exit
    mov ah, 0x4C
    int 0x21

; Print null-terminated string using BIOS teletype (AH=0Eh)
; Input: SI = pointer to string, BL = color
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
block_col db 0
block_row db 0
start_x   dw 0
start_y   dw 0

msg_title db "VGA Mode 13h - 256 Color Palette", 0
msg_info  db "320x200, 1 Byte Per Pixel (Linear)", 0
