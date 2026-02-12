; EGA Graphics Mode 0x0D Test
; 320x200, 16 Colors (planar, 4 bit planes at A000:0000)
; Each byte covers 8 pixels; 40 bytes per row
; Displays all 16 EGA colors with labels inside each swatch
; Tests: direct planar writes, AH=0Eh (transparent teletype)

org 0x100

start:
    ; Switch to EGA mode 0x0D (320x200, 16 colors)
    mov ah, 0x00
    mov al, 0x0D
    int 0x10

    ; Set up video segment for direct memory access
    mov ax, 0xA000
    mov es, ax

    ; Draw top row: colors 0-7
    ; Each box is 5 bytes wide (40 pixels) x 48 rows tall
    ; 8 boxes x 5 bytes = 40 bytes = full screen width
    mov byte [box_color], 0
    mov word [box_col], 0
.top_loop:
    mov word [box_row], 0
    call draw_box
    inc byte [box_color]
    add word [box_col], 5
    cmp byte [box_color], 8
    jb .top_loop

    ; Draw bottom row: colors 8-15
    mov word [box_col], 0
.bottom_loop:
    mov word [box_row], 128
    call draw_box
    inc byte [box_color]
    add word [box_col], 5
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
    ; AH=0Eh (teletype) draws transparent characters in graphics mode,
    ; preserving the colored box underneath while drawing text on top.
    ; Character grid: 40 cols x 25 rows (8x8 pixels per cell)
    ; Top boxes span char rows 0-5, bottom boxes span char rows 16-24
    ; Each box is 5 chars wide; label centered at char col 2 within each box

    ; Top row labels (char row 2, inside top boxes)
    mov byte [label_idx], 0
    mov byte [label_col], 2     ; Center of first box
.label_top:
    ; Set cursor
    mov ah, 0x02
    mov bh, 0
    mov dh, 2                   ; Char row 2 (pixel row 16)
    mov dl, [label_col]
    int 0x10
    ; Write hex digit via AH=0Eh (transparent teletype)
    xor ah, ah
    mov al, [label_idx]
    mov si, ax
    mov al, [hex_digits + si]
    mov ah, 0x0E
    mov bh, 0
    ; Use contrasting color: white on dark colors, black on light
    mov bl, [label_idx]
    cmp bl, 6                   ; Colors 6+ (brown, gray) are lighter
    jb .top_use_white
    mov bl, 0                   ; Black text
    jmp .top_write
.top_use_white:
    mov bl, 15                  ; White text
.top_write:
    int 0x10
    add byte [label_col], 5
    inc byte [label_idx]
    cmp byte [label_idx], 8
    jb .label_top

    ; Bottom row labels (char row 18, inside bottom boxes)
    mov byte [label_col], 1     ; Offset for 2-char labels
.label_bot:
    ; Set cursor
    mov ah, 0x02
    mov bh, 0
    mov dh, 18                  ; Char row 18 (pixel row 144)
    mov dl, [label_col]
    int 0x10
    ; Write 2-char hex label via AH=0Eh (transparent teletype)
    xor ah, ah
    mov al, [label_idx]
    mov si, ax
    sub si, 8                   ; Index into bot labels (0-7)
    shl si, 1                   ; 2 chars per label
    mov al, [bot_labels + si]
    mov ah, 0x0E
    mov bh, 0
    ; Use contrasting color: white on dark (8), black on light (9-15)
    mov bl, [label_idx]
    cmp bl, 9                   ; Color 8 (dark gray) is dark
    jae .bot_use_black
    mov bl, 15                  ; White text
    jmp .bot_write1
.bot_use_black:
    mov bl, 0                   ; Black text
.bot_write1:
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
    mov bl, [label_idx]
    cmp bl, 9
    jae .bot_use_black2
    mov bl, 15
    jmp .bot_write2
.bot_use_black2:
    mov bl, 0
.bot_write2:
    int 0x10
    add byte [label_col], 5
    inc byte [label_idx]
    cmp byte [label_idx], 16
    jb .label_bot

    ; --- Middle text area (char rows 7-14) ---

    ; Print header (char row 8)
    mov ah, 0x02
    mov bh, 0
    mov dh, 8
    mov dl, 7
    int 0x10
    mov si, msg_header
    mov bl, 15             ; White
    call print_string

    ; Print info (char row 10)
    mov ah, 0x02
    mov bh, 0
    mov dh, 10
    mov dl, 9
    int 0x10
    mov si, msg_info
    mov bl, 11             ; Light cyan
    call print_string

    ; Print color name legend - top row (char row 12)
    mov ah, 0x02
    mov bh, 0
    mov dh, 12
    mov dl, 0
    int 0x10
    mov si, msg_colors_top
    mov bl, 7              ; Light gray
    call print_string

    ; Print color name legend - bottom row (char row 14)
    mov ah, 0x02
    mov bh, 0
    mov dh, 14
    mov dl, 0
    int 0x10
    mov si, msg_colors_bot
    mov bl, 7              ; Light gray
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
    int 0x21

; Print null-terminated string using BIOS teletype (AH=0Eh, transparent)
; Input: SI = pointer to string, BL = color
print_string:
    pusha
.loop:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0E        ; Teletype output
    int 0x10
    jmp .loop
.done:
    popa
    ret

; Draw a 5-byte wide (40 pixel), 48-row tall box using EGA planar writes
; Parameters: box_row, box_col (byte offset 0-39), box_color (map mask)
; EGA memory is linear: offset = row * 40 + col
draw_box:
    pusha

    ; First clear all planes in box area (write 0x00 to all planes)
    mov dx, 0x3C4
    mov al, 0x02        ; Select Map Mask register
    out dx, al
    mov dx, 0x3C5
    mov al, 0x0F        ; All planes
    out dx, al

    mov cx, 48              ; 48 rows
    mov si, [box_row]       ; Starting row
.clear_loop:
    push cx
    mov ax, si
    mov bx, 40
    mul bx
    add ax, [box_col]
    mov di, ax
    mov cx, 5
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

    mov cx, 48              ; 48 rows
    mov si, [box_row]       ; Starting row
.row_loop:
    push cx
    mov ax, si
    mov bx, 40
    mul bx
    add ax, [box_col]
    mov di, ax
    mov cx, 5
    mov al, 0xFF            ; All pixels on
.write_loop:
    mov [es:di], al
    inc di
    loop .write_loop
    inc si
    pop cx
    loop .row_loop

.skip_draw:
    popa
    ret

; Data
box_row dw 0
box_col dw 0
box_color db 0
label_idx db 0
label_col db 0

hex_digits db "0123456789ABCDEF"
; 2-char labels for colors 8-15
bot_labels db " 8", " 9", "10", "11", "12", "13", "14", "15"

; Messages
msg_header     db "EGA Mode 0x0D - 16 Colors", 0
msg_info       db "320x200, 4 Bit Planes", 0
msg_colors_top db "Blk Blu Grn Cyn Red Mag Brn Gry", 0
msg_colors_bot db "DGy LBl LGn LCn LRd LMg Yel Wht", 0
