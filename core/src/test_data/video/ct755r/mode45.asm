; CGA Mode 4/5 Test
; Ported from CT755r/CT.ASM - mode45 procedure, standalone COM version.
; Tests CGA graphics modes 4 and 5 with palette/background variations.
;
; For each mode (4, then 5), 16 screens are shown:
;   Screens  1-8: drawn via INT 10h write pixel (AH=0Ch)
;   Screens  9-16: drawn via direct CGA VRAM write
; Each screen waits for a keypress.

[CPU 8086]
org 0x100
BITS 16

start:
    mov ah, 0x0F
    int 0x10
    mov [init_video_mode], al

    mov al, 4
    call mode45

    mov al, 5
    call mode45

    mov ah, 0x4C
    xor al, al
    int 0x21

; ============================================================
; mode45 - test one CGA graphics mode
; Input: AL = mode number (4 or 5)
; ============================================================
mode45:
    mov ah, 0x00
    mov [set_video_mode], al
    int 0x10

    ; Black background
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x00
    int 0x10

    ; Palette 0
    mov ah, 0x0B
    mov bh, 0x01
    mov bl, 0x00
    int 0x10

    ; Draw color stripes using INT 10h write pixel (AH=0Ch)
    ; rows  0-49 : background (cleared by mode set)
    ; rows 50-99 : color 1
    ; rows 100-149: color 2
    ; rows 150-199: color 3
    mov ah, 0x0C
    mov al, 1           ; CGA_COLOR1
    xor cx, cx          ; column 0
    mov dx, 50          ; start row

.draw_loop:
    push ax
    int 0x10
    pop ax
    inc cx
    cmp cx, 320
    jne .draw_loop
    xor cx, cx
    inc dx

    cmp dx, 100
    jne .not100
    mov al, 2
.not100:
    cmp dx, 150
    jne .not150
    mov al, 3
.not150:
    cmp dx, 200
    jne .draw_loop

    ; ---- Screen 1: Pal 0, Black bg ----
    xor dx, dx
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s1_m5
    mov si, mode4_pal0_s
    jmp .s1_mode
.s1_m5:
    mov si, mode5_pal0_s
.s1_mode:
    call print_str

    mov dx, 0x0200
    call set_cursor
    mov si, back_black_str
    call print_str

    mov dx, 0x0800
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s1_c1_m5
    mov si, pal0_c1_str
    jmp .s1_c1
.s1_c1_m5:
    mov si, color1_str
.s1_c1:
    call print_str

    mov dx, 0x0E00
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s1_c2_m5
    mov si, pal0_c2_str
    jmp .s1_c2
.s1_c2_m5:
    mov si, color2_str
.s1_c2:
    call print_str

    mov dx, 0x1400
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s1_c3_m5
    mov si, pal0_c3_str
    jmp .s1_c3
.s1_c3_m5:
    mov si, color3_str
.s1_c3:
    call print_str
    call wait_key

    ; ---- Screen 2: Pal 0, Blue bg ----
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x01
    int 0x10

    mov dx, 0x0200
    call set_cursor
    mov si, back_blue_str
    call print_str
    call wait_key

    ; ---- Screen 3: Pal 0, Blue HI bg ----
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x09
    int 0x10

    mov dx, 0x0200
    call set_cursor
    mov si, back_blueh_str
    call print_str
    call wait_key

    ; ---- Screen 4: Pal 0 HI, Blue HI bg ----
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x19
    int 0x10

    xor dx, dx
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s4_m5
    mov si, mode4_pal0h_s
    jmp .s4_mode
.s4_m5:
    mov si, mode5_pal0h_s
.s4_mode:
    call print_str

    mov dx, 0x0800
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s4_c1_m5
    mov si, pal0h_c1_str
    jmp .s4_c1
.s4_c1_m5:
    mov si, color1h_str
.s4_c1:
    call print_str

    mov dx, 0x0E00
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s4_c2_m5
    mov si, pal0h_c2_str
    jmp .s4_c2
.s4_c2_m5:
    mov si, color2h_str
.s4_c2:
    call print_str

    mov dx, 0x1400
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s4_c3_m5
    mov si, pal0h_c3_str
    jmp .s4_c3
.s4_c3_m5:
    mov si, color3h_str
.s4_c3:
    call print_str
    call wait_key

    ; Switch to Palette 1
    mov ah, 0x0B
    mov bh, 0x01
    mov bl, 0x01
    int 0x10

    ; ---- Screen 5: Pal 1, Black bg ----
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x00
    int 0x10

    xor dx, dx
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s5_m5
    mov si, mode4_pal1_s
    jmp .s5_mode
.s5_m5:
    mov si, mode5_pal1_s
.s5_mode:
    call print_str

    mov dx, 0x0200
    call set_cursor
    mov si, back_black_str
    call print_str

    mov dx, 0x0800
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s5_c1_m5
    mov si, pal1_c1_str
    jmp .s5_c1
.s5_c1_m5:
    mov si, color1_str
.s5_c1:
    call print_str

    mov dx, 0x0E00
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s5_c2_m5
    mov si, pal1_c2_str
    jmp .s5_c2
.s5_c2_m5:
    mov si, color2_str
.s5_c2:
    call print_str

    mov dx, 0x1400
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s5_c3_m5
    mov si, pal1_c3_str
    jmp .s5_c3
.s5_c3_m5:
    mov si, color3_str
.s5_c3:
    call print_str
    call wait_key

    ; ---- Screen 6: Pal 1, Blue bg ----
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x01
    int 0x10

    mov dx, 0x0200
    call set_cursor
    mov si, back_blue_str
    call print_str
    call wait_key

    ; ---- Screen 7: Pal 1, Blue HI bg ----
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x09
    int 0x10

    mov dx, 0x0200
    call set_cursor
    mov si, back_blueh_str
    call print_str
    call wait_key

    ; ---- Screen 8: Pal 1 HI, Blue HI bg ----
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x19
    int 0x10

    xor dx, dx
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s8_m5
    mov si, mode4_pal1h_s
    jmp .s8_mode
.s8_m5:
    mov si, mode5_pal1h_s
.s8_mode:
    call print_str

    mov dx, 0x0800
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s8_c1_m5
    mov si, pal1h_c1_str
    jmp .s8_c1
.s8_c1_m5:
    mov si, color1h_str
.s8_c1:
    call print_str

    mov dx, 0x0E00
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s8_c2_m5
    mov si, pal1h_c2_str
    jmp .s8_c2
.s8_c2_m5:
    mov si, color2h_str
.s8_c2:
    call print_str

    mov dx, 0x1400
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s8_c3_m5
    mov si, pal1h_c3_str
    jmp .s8_c3
.s8_c3_m5:
    mov si, color3h_str
.s8_c3:
    call print_str
    call wait_key

    ; ============================================================
    ; Screens 9-16: same pattern via direct CGA VRAM / port writes
    ; ============================================================

    ; Draw color stripes directly into CGA VRAM
    mov ax, 0xB800
    mov es, ax
    xor al, al          ; color 0
    xor si, si          ; even-row bank  (0x0000)
    mov di, 0x2000      ; odd-row  bank  (0x2000)

.direct_draw:
    mov byte [es:si], al
    mov byte [es:di], al
    inc si
    inc di
    cmp si, 2000
    jne .dd2
    mov al, 0x55        ; 01010101 = color 1 x4 pixels
.dd2:
    cmp si, 4000
    jne .dd3
    mov al, 0xAA        ; 10101010 = color 2 x4 pixels
.dd3:
    cmp si, 6000
    jne .dd4
    mov al, 0xFF        ; 11111111 = color 3 x4 pixels
.dd4:
    cmp si, 8000
    jb .direct_draw

    ; ---- Screen 9: Direct, Pal 0, Black bg ----
    mov ax, 0x0040
    mov es, ax
    mov dx, 0x3D9
    mov al, 0x00
    out dx, al
    mov byte [es:0x66], al

    xor dx, dx
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s9_m5
    mov si, mode4_pal0_dw_s
    jmp .s9_mode
.s9_m5:
    mov si, mode5_pal0_dw_s
.s9_mode:
    call print_str

    mov dx, 0x0200
    call set_cursor
    mov si, back_black_str
    call print_str

    mov dx, 0x0800
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s9_c1_m5
    mov si, pal0_c1_str
    jmp .s9_c1
.s9_c1_m5:
    mov si, color1_str
.s9_c1:
    call print_str

    mov dx, 0x0E00
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s9_c2_m5
    mov si, pal0_c2_str
    jmp .s9_c2
.s9_c2_m5:
    mov si, color2_str
.s9_c2:
    call print_str

    mov dx, 0x1400
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s9_c3_m5
    mov si, pal0_c3_str
    jmp .s9_c3
.s9_c3_m5:
    mov si, color3_str
.s9_c3:
    call print_str
    call wait_key

    ; ---- Screen 10: Direct, Pal 0, Blue bg ----
    mov ax, 0x0040
    mov es, ax
    mov dx, 0x3D9
    mov al, 0x01
    out dx, al
    mov byte [es:0x66], al

    mov dx, 0x0200
    call set_cursor
    mov si, back_blue_str
    call print_str
    call wait_key

    ; ---- Screen 11: Direct, Pal 0, Blue HI bg ----
    mov ax, 0x0040
    mov es, ax
    mov dx, 0x3D9
    mov al, 0x09
    out dx, al
    mov byte [es:0x66], al

    mov dx, 0x0200
    call set_cursor
    mov si, back_blueh_str
    call print_str
    call wait_key

    ; ---- Screen 12: Direct, Pal 0 HI, Blue HI bg ----
    mov ax, 0x0040
    mov es, ax
    mov dx, 0x3D9
    mov al, 0x19
    out dx, al
    mov byte [es:0x66], al

    xor dx, dx
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s12_m5
    mov si, mode4_pal0h_dw_s
    jmp .s12_mode
.s12_m5:
    mov si, mode5_pal0h_dw_s
.s12_mode:
    call print_str

    mov dx, 0x0800
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s12_c1_m5
    mov si, pal0h_c1_str
    jmp .s12_c1
.s12_c1_m5:
    mov si, color1h_str
.s12_c1:
    call print_str

    mov dx, 0x0E00
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s12_c2_m5
    mov si, pal0h_c2_str
    jmp .s12_c2
.s12_c2_m5:
    mov si, color2h_str
.s12_c2:
    call print_str

    mov dx, 0x1400
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s12_c3_m5
    mov si, pal0h_c3_str
    jmp .s12_c3
.s12_c3_m5:
    mov si, color3h_str
.s12_c3:
    call print_str
    call wait_key

    ; ---- Screen 13: Direct, Pal 1, Black bg ----
    mov ax, 0x0040
    mov es, ax
    mov dx, 0x3D9
    mov al, 0x20
    out dx, al
    mov byte [es:0x66], al

    xor dx, dx
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s13_m5
    mov si, mode4_pal1_dw_s
    jmp .s13_mode
.s13_m5:
    mov si, mode5_pal1_dw_s
.s13_mode:
    call print_str

    mov dx, 0x0200
    call set_cursor
    mov si, back_black_str
    call print_str

    mov dx, 0x0800
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s13_c1_m5
    mov si, pal1_c1_str
    jmp .s13_c1
.s13_c1_m5:
    mov si, color1_str
.s13_c1:
    call print_str

    mov dx, 0x0E00
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s13_c2_m5
    mov si, pal1_c2_str
    jmp .s13_c2
.s13_c2_m5:
    mov si, color2_str
.s13_c2:
    call print_str

    mov dx, 0x1400
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s13_c3_m5
    mov si, pal1_c3_str
    jmp .s13_c3
.s13_c3_m5:
    mov si, color3_str
.s13_c3:
    call print_str
    call wait_key

    ; ---- Screen 14: Direct, Pal 1, Blue bg ----
    mov ax, 0x0040
    mov es, ax
    mov dx, 0x3D9
    mov al, 0x21
    out dx, al
    mov byte [es:0x66], al

    mov dx, 0x0200
    call set_cursor
    mov si, back_blue_str
    call print_str
    call wait_key

    ; ---- Screen 15: Direct, Pal 1, Blue HI bg ----
    mov ax, 0x0040
    mov es, ax
    mov dx, 0x3D9
    mov al, 0x29
    out dx, al
    mov byte [es:0x66], al

    mov dx, 0x0200
    call set_cursor
    mov si, back_blueh_str
    call print_str
    call wait_key

    ; ---- Screen 16: Direct, Pal 1 HI, Blue HI bg ----
    mov ax, 0x0040
    mov es, ax
    mov dx, 0x3D9
    mov al, 0x39
    out dx, al
    mov byte [es:0x66], al

    xor dx, dx
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s16_m5
    mov si, mode4_pal1h_dw_s
    jmp .s16_mode
.s16_m5:
    mov si, mode5_pal1h_dw_s
.s16_mode:
    call print_str

    mov dx, 0x0800
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s16_c1_m5
    mov si, pal1h_c1_str
    jmp .s16_c1
.s16_c1_m5:
    mov si, color1h_str
.s16_c1:
    call print_str

    mov dx, 0x0E00
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s16_c2_m5
    mov si, pal1h_c2_str
    jmp .s16_c2
.s16_c2_m5:
    mov si, color2h_str
.s16_c2:
    call print_str

    mov dx, 0x1400
    call set_cursor
    cmp byte [set_video_mode], 4
    jne .s16_c3_m5
    mov si, pal1h_c3_str
    jmp .s16_c3
.s16_c3_m5:
    mov si, color3h_str
.s16_c3:
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

init_video_mode     db 0
set_video_mode      db 0

mode4_pal0_s        db "CGA mode 4, Palette 0   ", 0
mode4_pal0h_s       db "CGA mode 4, Palette 0 HI", 0
mode4_pal1_s        db "CGA mode 4, Palette 1   ", 0
mode4_pal1h_s       db "CGA mode 4, Palette 1 HI", 0

mode5_pal0_s        db "CGA mode 5, Palette 0   ", 0
mode5_pal0h_s       db "CGA mode 5, Palette 0 HI", 0
mode5_pal1_s        db "CGA mode 5, Palette 1   ", 0
mode5_pal1h_s       db "CGA mode 5, Palette 1 HI", 0

mode4_pal0_dw_s     db "CGA mode 4, Palette 0     Direct write", 0
mode4_pal0h_dw_s    db "CGA mode 4, Palette 0 HI  Direct write", 0
mode4_pal1_dw_s     db "CGA mode 4, Palette 1     Direct write", 0
mode4_pal1h_dw_s    db "CGA mode 4, Palette 1 HI  Direct write", 0

mode5_pal0_dw_s     db "CGA mode 5, Palette 0     Direct write", 0
mode5_pal0h_dw_s    db "CGA mode 5, Palette 0 HI  Direct write", 0
mode5_pal1_dw_s     db "CGA mode 5, Palette 1     Direct write", 0
mode5_pal1h_dw_s    db "CGA mode 5, Palette 1 HI  Direct write", 0

back_black_str      db "Background Color Black  ", 0
back_blue_str       db "Background Color Blue   ", 0
back_blueh_str      db "Background Color Blue HI", 0

; Mode 4 palette color names
pal0_c1_str         db "Green     ", 0
pal0_c2_str         db "Red       ", 0
pal0_c3_str         db "Brown     ", 0
pal0h_c1_str        db "Green   HI", 0
pal0h_c2_str        db "Red     HI", 0
pal0h_c3_str        db "Yellow    ", 0

pal1_c1_str         db "Cyan      ", 0
pal1_c2_str         db "Magenta   ", 0
pal1_c3_str         db "Gray      ", 0
pal1h_c1_str        db "Cyan    HI", 0
pal1h_c2_str        db "Magenta HI", 0
pal1h_c3_str        db "White     ", 0

; Mode 5 color names
color1_str          db "Cyan      ", 0
color2_str          db "Red       ", 0
color3_str          db "Grey      ", 0
color1h_str         db "Cyan    HI", 0
color2h_str         db "Red     HI", 0
color3h_str         db "White     ", 0
