; CGA Mode 6 Test
; Ported from CT755r/CT.ASM - mode6 procedure, standalone COM version.
; Tests CGA graphics mode 6 (640x200 B/W) with foreground color variations.
;
; 6 screens are shown:
;   Screens 1-3: drawn via INT 10h write pixel (AH=0Ch)
;     Screen 1: B/W (white foreground)
;     Screen 2: Green foreground
;     Screen 3: Green HI foreground
;   Screens 4-6: drawn via direct CGA VRAM write
;     Screen 4: White HI foreground (direct write)
;     Screen 5: Green foreground (direct write)
;     Screen 6: Green HI foreground (direct write)
; Each screen waits for a keypress.

[CPU 8086]
org 0x100
BITS 16

start:
    mov ah, 0x0F
    int 0x10
    mov [init_video_mode], al

    mov al, 6
    call mode6

    mov ah, 0x4C
    xor al, al
    int 0x21

; ============================================================
; mode6 - test CGA graphics mode 6 (640x200 B/W)
; ============================================================
mode6:
    mov ah, 0x00
    int 0x10

    ; Draw horizontal stripes via INT 10h write pixel (AH=0Ch)
    ; Stripes at rows: 0-19, 40-59, 80-99, 120-139, 160-179
    mov ah, 0x0C
    mov al, 0x01        ; foreground color (white in mode 6)
    xor cx, cx          ; column 0
    xor dx, dx          ; row 0

.draw_loop:
    push ax
    int 0x10
    pop ax
    inc cx
    cmp cx, 640
    jne .draw_loop
    xor cx, cx
    inc dx

    cmp dx, 20
    jne .ck60
    mov dx, 40
.ck60:
    cmp dx, 60
    jne .ck100
    mov dx, 80
.ck100:
    cmp dx, 100
    jne .ck140
    mov dx, 120
.ck140:
    cmp dx, 140
    jne .ck180
    mov dx, 160
.ck180:
    cmp dx, 180
    jb .draw_loop

    ; ---- Screen 1: B/W (white foreground via INT 10h) ----
    xor dx, dx
    call set_cursor
    mov si, mode6_s
    call print_str
    call wait_key

    ; Set green as foreground color
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x02
    int 0x10

    ; ---- Screen 2: Green foreground ----
    xor dx, dx
    call set_cursor
    mov si, mode6_green_s
    call print_str
    call wait_key

    ; Set green HI as foreground color
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x0A
    int 0x10

    ; ---- Screen 3: Green HI foreground ----
    xor dx, dx
    call set_cursor
    mov si, mode6_green_hi_s
    call print_str
    call wait_key

    ; ============================================================
    ; Screens 4-6: direct CGA VRAM write
    ; ============================================================

    ; Set white HI as foreground color via direct I/O
    mov ax, 0x0040
    mov es, ax
    mov dx, 0x3D9
    mov al, 0x0F            ; white HI foreground
    out dx, al
    mov byte [es:0x66], al

    ; Draw horizontal stripes directly into CGA VRAM
    ; Even rows at 0xB800:0x0000, odd rows at 0xB800:0x2000
    ; 80 bytes/row * 10 rows/stripe = 800 bytes/stripe; 10 stripes total
    mov ax, 0xB800
    mov es, ax
    mov al, 0xFF            ; start with pixels ON
    xor si, si              ; even-row bank (0x0000)
    mov di, 0x2000          ; odd-row bank  (0x2000)

.direct_draw:
    mov byte [es:si], al
    mov byte [es:di], al
    inc si
    inc di
    cmp si, 800
    jne .dd2
    xor al, 0xFF
.dd2:
    cmp si, 1600
    jne .dd3
    xor al, 0xFF
.dd3:
    cmp si, 2400
    jne .dd4
    xor al, 0xFF
.dd4:
    cmp si, 3200
    jne .dd5
    xor al, 0xFF
.dd5:
    cmp si, 4000
    jne .dd6
    xor al, 0xFF
.dd6:
    cmp si, 4800
    jne .dd7
    xor al, 0xFF
.dd7:
    cmp si, 5600
    jne .dd8
    xor al, 0xFF
.dd8:
    cmp si, 6400
    jne .dd9
    xor al, 0xFF
.dd9:
    cmp si, 7200
    jne .dd10
    xor al, 0xFF
.dd10:
    cmp si, 8000
    jb .direct_draw

    ; ---- Screen 4: Direct write, White HI foreground ----
    xor dx, dx
    call set_cursor
    mov si, mode6_dw_s
    call print_str
    call wait_key

    ; Set green as foreground color (direct I/O)
    mov dx, 0x3D9
    mov al, 0x02
    out dx, al

    ; ---- Screen 5: Direct write, Green foreground ----
    xor dx, dx
    call set_cursor
    mov si, mode6_green_dw_s
    call print_str
    call wait_key

    ; Set green HI as foreground color (direct I/O)
    mov dx, 0x3D9
    mov al, 0x0A
    out dx, al

    ; ---- Screen 6: Direct write, Green HI foreground ----
    xor dx, dx
    call set_cursor
    mov si, mode6_green_dw_hi_s
    call print_str
    call wait_key

    ; Restore initial video mode
    xor ah, ah
    mov al, [init_video_mode]
    int 0x10

    ; sync_fix (EGA installation check, suppresses sync issue on G3101)
    mov ah, 0x12
    mov bx, 0xFF10
    int 0x10

    ret

; ============================================================
; Subroutines
; ============================================================

set_cursor:
    push ax
    push bx
    mov ah, 0x02
    mov bh, 0x00
    int 0x10
    pop bx
    pop ax
    ret

wait_key:
    push es
    push ax
    cli
    mov ax, 0x40
    mov es, ax
    mov al, [es:0x1A]
    mov [es:0x1C], al       ; flush keyboard buffer
    sti
    pop ax
    pop es
    xor ah, ah
    int 0x16
    ret

print_str:
    push ax
    push bx
.loop:
    lodsb
    cmp al, 0
    je .done
    mov bx, 0x0001
    mov ah, 0x0E
    int 0x10
    jmp .loop
.done:
    pop bx
    pop ax
    ret

; ============================================================
; Data
; ============================================================

init_video_mode         db 0

mode6_s                 db "CGA mode 6 B/W                       ", 0
mode6_green_s           db "CGA mode 6, Foreground Color Green   ", 0
mode6_green_hi_s        db "CGA mode 6, Foreground Color Green HI", 0
mode6_dw_s              db "CGA mode 6 B/W                         Direct write", 0
mode6_green_dw_s        db "CGA mode 6, Foreground Color Green     Direct write", 0
mode6_green_dw_hi_s     db "CGA mode 6, Foreground Color Green HI  Direct write", 0
