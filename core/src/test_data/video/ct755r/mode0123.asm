; CGA Text Mode 0-3 Test
; Ported from CT.ASM (CT755r CGA test suite), mode0123 procedure.
; Tests CGA text modes 0 (40x25 BW), 1 (40x25 color), 2 (80x25 BW), 3 (80x25 color).
;
; For each mode:
;   Screen 1: all 256 ASCII characters in a 16x16 grid (row 2, col 2)
;   Screen 2: 16 color bars (rows 4-19) with color name labels
; Each screen waits for a keypress before continuing.

[CPU 8086]
org 0x100
BITS 16

start:
    ; Save initial video mode
    mov ah, 0x0F
    int 0x10
    mov [init_video_mode], al

    ; Test all four text modes in order
    mov al, 0
    call mode0123
    mov al, 1
    call mode0123
    mov al, 2
    call mode0123
    mov al, 3
    call mode0123

    ; Restore initial video mode and exit
    xor ah, ah
    mov al, [init_video_mode]
    int 0x10

    mov ah, 0x4C
    xor al, al
    int 0x21

;------------------------------------------------------------------------------
; mode0123 - test one CGA text mode
; Input: AL = mode number (0, 1, 2, or 3)
;------------------------------------------------------------------------------
mode0123:
    ; Set video mode
    xor ah, ah
    push ax
    mov [set_video_mode], al
    int 0x10
    pop ax

    ; sym_row: 40 columns for modes 0/1, 80 for modes 2/3
    mov word [sym_row], 40
    cmp al, 2
    jb .sym_row_set
    mov word [sym_row], 80
.sym_row_set:

    ; Draw all 256 ASCII chars in a 16x16 grid starting at row 2, col 2
    mov dh, 2               ; row
    mov dl, 2               ; col
    call set_cursor
    mov al, 0x00            ; starting character
    mov bh, 0x00            ; page 0
    mov bl, 0x07            ; attribute (white on black)

.write_rows:
    mov cx, 16
.write_cols:
    push dx
    push ax
    mov ah, 0x09
    ; CX doubles as INT 10h repeat count and loop counter.
    ; Each iteration writes CX copies (16..1) starting at the current cursor,
    ; filling rightward. The next iteration writes one fewer copy one column
    ; to the right, preserving the previous char. Net result: one char per cell.
    int 0x10                ; write char+attr at cursor (no advance)
    pop ax
    pop dx
    inc al
    inc dl
    call set_cursor
    loop .write_cols

    inc dh
    mov dl, 2
    call set_cursor
    cmp dh, (16 + 2)
    jb .write_rows

    ; Hide cursor (move off screen)
    mov dh, 25
    mov dl, 0
    call set_cursor

    call wait_key

    ;--- Second screen: 16 color bars with labels ---
    ; Reset mode to clear the screen
    xor ah, ah
    mov al, [set_video_mode]
    int 0x10

    mov dh, 4               ; start at row 4
    mov dl, 0               ; column 0
    mov bl, 0               ; starting attribute (color 0)
    mov cx, 16              ; 16 colors

.color_loop:
    push cx
    push dx
    push bx

    call set_cursor
    mov cx, [sym_row]       ; repeat count = 40 or 80
    mov bh, 0x00            ; page 0
    mov al, 219             ; full block char (█)

    ; Color 0 (Black) is invisible as-is: use white attr + space char
    cmp bl, 0x00
    jne .not_black
    mov bl, 0x07            ; white on black attribute
    mov al, 0x00            ; space char (prints as black)
.not_black:

    ; Color 8 (HI Black) needs special handling: white bg with block char
    cmp bl, 0x08
    jne .not_hiblack
    mov bl, 0x78            ; white bg, hi-black fg
    mov al, 219
.not_hiblack:

    mov ah, 0x09
    int 0x10                ; write chars with attribute

    pop bx
    pop dx
    pop cx

    inc dh                  ; next row
    inc bl                  ; next color
    loop .color_loop

    ; Print color name labels over the bars (row 4, col 0)
    mov dh, 4
    mov dl, 0
    call set_cursor

    mov si, black_str
    call print_str
    mov si, blue_str
    call print_str
    mov si, green_str
    call print_str
    mov si, cyan_str
    call print_str
    mov si, red_str
    call print_str
    mov si, magenta_str
    call print_str
    mov si, brown_str
    call print_str
    mov si, gray_str
    call print_str
    mov si, hi_black_str
    call print_str
    mov si, hi_blue_str
    call print_str
    mov si, hi_green_str
    call print_str
    mov si, hi_cyan_str
    call print_str
    mov si, hi_red_str
    call print_str
    mov si, hi_magenta_str
    call print_str
    mov si, yellow_str
    call print_str
    mov si, white_str
    call print_str

    ; Hide cursor
    mov dh, 25
    mov dl, 0
    call set_cursor

    call wait_key
    ret

;------------------------------------------------------------------------------
; set_cursor - position cursor
; Input: DH = row, DL = column
; Preserves: AX
;------------------------------------------------------------------------------
set_cursor:
    push ax
    mov ah, 0x02
    mov bh, 0               ; page 0
    int 0x10
    pop ax
    ret

;------------------------------------------------------------------------------
; wait_key - flush keyboard buffer then wait for a keypress
;------------------------------------------------------------------------------
wait_key:
    push es
    cli
    mov ax, 0x40
    mov es, ax
    mov al, [es:0x1A]       ; head pointer
    mov [es:0x1C], al       ; move tail to head (flush)
    sti
    pop es
    xor ah, ah
    int 0x16                ; wait for keypress
    ret

;------------------------------------------------------------------------------
; print_str - print null-terminated string via INT 10h teletype
; Input: SI = pointer to string
;------------------------------------------------------------------------------
print_str:
    push ax
    push bx
.loop:
    lodsb
    cmp al, 0
    je .done
    mov bx, 0x0001          ; page 0, fg color 1
    mov ah, 0x0E
    int 0x10
    jmp .loop
.done:
    pop bx
    pop ax
    ret

;------------------------------------------------------------------------------
; Data
;------------------------------------------------------------------------------
init_video_mode db 0
set_video_mode  db 0
sym_row         dw 40

; Color name strings for text modes (with CR+LF for teletype newline)
black_str       db "Black      ", 0x0D, 0x0A, 0
blue_str        db "Blue       ", 0x0D, 0x0A, 0
green_str       db "Green      ", 0x0D, 0x0A, 0
cyan_str        db "Cyan       ", 0x0D, 0x0A, 0
red_str         db "Red        ", 0x0D, 0x0A, 0
magenta_str     db "Magenta    ", 0x0D, 0x0A, 0
brown_str       db "Brown      ", 0x0D, 0x0A, 0
gray_str        db "Gray       ", 0x0D, 0x0A, 0
hi_black_str    db "HI Black   ", 0x0D, 0x0A, 0
hi_blue_str     db "HI Blue    ", 0x0D, 0x0A, 0
hi_green_str    db "HI Green   ", 0x0D, 0x0A, 0
hi_cyan_str     db "HI Cyan    ", 0x0D, 0x0A, 0
hi_red_str      db "HI Red     ", 0x0D, 0x0A, 0
hi_magenta_str  db "HI Magenta ", 0x0D, 0x0A, 0
yellow_str      db "Yellow     ", 0x0D, 0x0A, 0
white_str       db "White      ", 0x0D, 0x0A, 0
