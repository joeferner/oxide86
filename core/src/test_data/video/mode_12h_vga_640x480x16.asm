; VGA Graphics Mode 0x12 Test
; 640x480, 16 Colors (planar, 4 bit planes at A000:0000)
; Each byte covers 8 pixels; 80 bytes per row
; Displays all 16 VGA colors with labels inside each swatch
; Tests: direct planar writes, AH=0Eh (transparent teletype)

[CPU 8086]
org 0x100

start:
    ; Switch to VGA mode 0x12 (640x480, 16 colors)
    mov ah, 0x00
    mov al, 0x12
    int 0x10

    ; Set up video segment for direct memory access
    mov ax, 0xA000
    mov es, ax

    ; Draw top row: colors 0-7
    ; Each box is 10 bytes wide (80 pixels) x 160 rows tall
    ; 8 boxes x 10 bytes = 80 bytes = full screen width
    mov byte [box_color], 0
    mov word [box_col], 0
.top_loop:
    mov word [box_row], 0
    call draw_box
    inc byte [box_color]
    add word [box_col], 10
    cmp byte [box_color], 8
    jb .top_loop

    ; Draw bottom row: colors 8-15
    mov word [box_col], 0
.bottom_loop:
    mov word [box_row], 320
    call draw_box
    inc byte [box_color]
    add word [box_col], 10
    cmp byte [box_color], 16
    jb .bottom_loop

    ; Restore map mask to all planes for BIOS text output
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    mov dx, 0x3C5
    mov al, 0x0F
    out dx, al

    ; --- Label each color box with its hex number using AH=0Eh ---
    ; Mode 12h character cell: 8x16 pixels -> 30 char rows, 80 char cols
    ; Top boxes span char rows 0-9, bottom boxes span char rows 20-29
    ; Each box is 10 chars wide; labels centered within each box

    ; Top row labels (char row 4, inside top boxes rows 0-159)
    mov byte [label_idx], 0
    mov byte [label_col], 4     ; Center of first box (cols 0-9)
.label_top:
    ; Set cursor
    mov ah, 0x02
    mov bh, 0
    mov dh, 4                   ; Char row 4 (pixel row 64)
    mov dl, [label_col]
    int 0x10
    ; Write hex digit via AH=0Eh (transparent teletype)
    xor ah, ah
    mov al, [label_idx]
    mov si, ax
    mov al, [hex_digits + si]
    mov ah, 0x0E
    mov bh, 0
    mov bl, 15                  ; White text
    int 0x10
    add byte [label_col], 10
    inc byte [label_idx]
    cmp byte [label_idx], 8
    jb .label_top

    ; Bottom row labels (char row 24, inside bottom boxes rows 320-479)
    mov byte [label_col], 3     ; Offset for 2-char labels
.label_bot:
    ; Set cursor
    mov ah, 0x02
    mov bh, 0
    mov dh, 24                  ; Char row 24 (pixel row 384)
    mov dl, [label_col]
    int 0x10
    ; Write first char of 2-char hex label
    xor ah, ah
    mov al, [label_idx]
    mov si, ax
    sub si, 8                   ; Index into bot labels (0-7)
    shl si, 1                   ; 2 chars per label
    mov al, [bot_labels + si]
    mov ah, 0x0E
    mov bh, 0
    mov bl, 15                  ; White text
    int 0x10
    ; Write second char (cursor already advanced by AH=0Eh)
    xor ah, ah
    mov al, [label_idx]
    mov si, ax
    sub si, 8
    shl si, 1
    mov al, [bot_labels + si + 1]
    mov ah, 0x0E
    mov bh, 0
    mov bl, 15
    int 0x10
    add byte [label_col], 10
    inc byte [label_idx]
    cmp byte [label_idx], 16
    jb .label_bot

    ; --- Middle text area (char rows 14-15) ---

    ; Print header (char row 14)
    mov ah, 0x02
    mov bh, 0
    mov dh, 14
    mov dl, 24
    int 0x10
    mov si, msg_header
    mov bl, 15                  ; White
    call print_string

    ; Print info (char row 15)
    mov ah, 0x02
    mov bh, 0
    mov dh, 15
    mov dl, 26
    int 0x10
    mov si, msg_info
    mov bl, 11                  ; Light cyan
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

; Print null-terminated string using BIOS teletype (AH=0Eh, transparent)
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

; Draw a 10-byte wide (80 pixel), 160-row tall box using VGA planar writes
; Parameters: box_row, box_col (byte offset 0-79), box_color (map mask)
; VGA memory layout: offset = row * 80 + col
draw_box:
    push ax
    push bx
    push cx
    push dx
    push si
    push di

    ; First clear all planes in box area (write 0x00 to all planes)
    mov dx, 0x3C4
    mov al, 0x02                ; Select Map Mask register
    out dx, al
    mov dx, 0x3C5
    mov al, 0x0F                ; All planes
    out dx, al

    mov cx, 160                 ; 160 rows
    mov si, [box_row]           ; Starting row
.clear_loop:
    push cx
    mov ax, si
    mov bx, 80
    mul bx
    add ax, [box_col]
    mov di, ax
    mov cx, 10
    xor al, al
.clear_inner:
    mov [es:di], al
    inc di
    loop .clear_inner
    inc si
    pop cx
    loop .clear_loop

    ; Now set map mask to desired color and write
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    mov dx, 0x3C5
    mov al, [box_color]
    out dx, al

    ; Skip drawing if color is 0 (black on black, already cleared)
    test al, al
    jz .skip_draw

    mov cx, 160                 ; 160 rows
    mov si, [box_row]           ; Starting row
.row_loop:
    push cx
    mov ax, si
    mov bx, 80
    mul bx
    add ax, [box_col]
    mov di, ax
    mov cx, 10
    mov al, 0xFF                ; All pixels on
.write_loop:
    mov [es:di], al
    inc di
    loop .write_loop
    inc si
    pop cx
    loop .row_loop

.skip_draw:
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Data
box_row   dw 0
box_col   dw 0
box_color db 0
label_idx db 0
label_col db 0

hex_digits db "0123456789ABCDEF"
; 2-char labels for colors 8-15
bot_labels db " 8", " 9", "10", "11", "12", "13", "14", "15"

; Messages
msg_header db "VGA Mode 0x12 - 16 Colors", 0
msg_info   db "640x480, 4 Bit Planes", 0
