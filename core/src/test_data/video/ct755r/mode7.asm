; MDA Mode 7 Test
; Ported from CT755r/MDAHGC.ASM - mode7 procedure, standalone COM version.
; Tests MDA text mode 7 (80x25 monochrome) with:
;   Screen 1: All 256 ASCII characters in a 16x16 grid (rows 2-17, cols 2-17)
;   Screen 2: All MDA attribute combinations with labeled text (rows 4-13)
; Each screen waits for a keypress.

[CPU 8086]
org 0x100
BITS 16

start:
    mov ah, 0x0F
    int 0x10
    mov [init_video_mode], al

    mov al, 7
    call mode7

    mov ah, 0x4C
    xor al, al
    int 0x21

; ============================================================
; mode7 - test MDA text mode 7 (80x25 monochrome)
; ============================================================
mode7:
    mov ah, 0x00
    mov [set_video_mode], al
    int 0x10

    ; ---- Screen 1: 256 ASCII chars in 16x16 grid ----
    mov dh, 2           ; cursor row
    mov dl, 2           ; cursor column
    call set_cursor
    mov al, 0x00        ; starting char
    mov bh, 0x00        ; page 0
    mov bl, 0x07        ; attribute: normal

.write_mda_row:
    mov cx, 16          ; 16 chars per row
.inner:
    push cx
    push dx
    push ax
    mov cx, 1           ; write 1 char at cursor position
    mov ah, 0x09
    int 0x10
    pop ax
    pop dx
    pop cx
    inc al
    inc dl
    call set_cursor
    loop .inner

    inc dh
    mov dl, 2
    call set_cursor
    cmp dh, 18          ; 16 rows + 2 (= rows 2..17 inclusive)
    jb .write_mda_row

    ; Hide cursor (row 25 is off-screen)
    mov dh, 25
    mov dl, 0
    call set_cursor
    call wait_key

    ; ---- Screen 2: MDA attribute combinations ----
    ; Set mode again to clear screen
    mov ah, 0x00
    mov al, [set_video_mode]
    int 0x10

    ; Write 40 spaces with each attribute at rows 4-13
    mov dh, 4
    mov dl, 0
    mov si, mda_attr
    mov cx, 10          ; 10 attributes

.bw_txt_loop:
    push cx
    push dx
    call set_cursor
    mov cx, 40
    mov bh, 0x00
    mov al, 0x00        ; null char (displays as space)
    mov bl, [si]
    inc si
    mov ah, 0x09
    int 0x10
    pop dx
    pop cx
    inc dh
    loop .bw_txt_loop

    ; Print text strings over the attribute rows, starting at row 4
    mov dh, 4
    mov dl, 0
    call set_cursor

    mov si, normal_str
    call print_str
    mov si, br_str
    call print_str
    mov si, und_str
    call print_str
    mov si, und_br_str
    call print_str
    mov si, neg_str
    call print_str
    mov si, bl_str
    call print_str
    mov si, bl_br_str
    call print_str
    mov si, bl_und_str
    call print_str
    mov si, bl_und_br_str
    call print_str
    mov si, bl_neg_str
    call print_str

    ; Hide cursor
    mov dh, 25
    mov dl, 0
    call set_cursor
    call wait_key

    ; Restore initial video mode
    mov ah, 0x00
    mov al, [init_video_mode]
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
    mov bx, 0x0001          ; page 0, color 1
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

mda_attr            db 0x07, 0x0F, 0x01, 0x09, 0x70, 0x87, 0x8F, 0x81, 0x89, 0xF0

normal_str          db "Normal Text                      ", 0x0D, 0x0A, 0
br_str              db "Bright Text                      ", 0x0D, 0x0A, 0
und_str             db "Underlined Text                  ", 0x0D, 0x0A, 0
und_br_str          db "Underlined Bright Text           ", 0x0D, 0x0A, 0
neg_str             db "Negative Text                    ", 0x0D, 0x0A, 0
bl_str              db "Blinking Text                    ", 0x0D, 0x0A, 0
bl_br_str           db "Blinking Bright Text             ", 0x0D, 0x0A, 0
bl_und_str          db "Blinking Underlined Text         ", 0x0D, 0x0A, 0
bl_und_br_str       db "Blinking Underlined Bright Text  ", 0x0D, 0x0A, 0
bl_neg_str          db "Blinking Negative Text           ", 0x0D, 0x0A, 0
