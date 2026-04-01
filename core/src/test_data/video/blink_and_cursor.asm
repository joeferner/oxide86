; Blink and Cursor Test
; Tests blinking text attributes and cursor positioning in CGA text mode 3.
;
;   Screen 1 — blink_and_cursor_01_blink_off.png
;     Intensity mode (blink OFF). Bit 7 = bright background.
;     Row 2: "Normal"       attr 0x07  -> light gray text, black bg
;     Row 3: "Bright BG"    attr 0x87  -> light gray text, dark gray bg
;     Row 4: "Bright Both"  attr 0x8F  -> white text, dark gray bg
;     Row 5: "Red BG"       attr 0xC7  -> light gray text, bright red bg
;     Cursor hidden (row 25). All text fully visible, no blinking.
;
;   Screen 2a — blink_and_cursor_02_blink_on_visible.png
;     Blink enabled, visible phase (blink_phase=false). Bit 7 = blink.
;     Row 2: "No Blink"     attr 0x07  -> light gray text, black bg (no blink bit)
;     Row 3: "Blink"        attr 0x87  -> light gray text, black bg (blink, visible)
;     Row 4: "Blink Bright" attr 0x8F  -> white text, black bg (blink, visible)
;     Row 5: "Blink Red"    attr 0xC7  -> light gray text, red bg (blink, visible)
;     Cursor hidden. All text visible (visible phase of blink cycle).
;
;   Screen 2b — blink_and_cursor_03_blink_on_blanked.png
;     Same VRAM as 2a, blanked phase (blink_phase=true).
;     Row 2: "No Blink"     attr 0x07  -> unchanged (no blink bit)
;     Row 3: "Blink"        attr 0x87  -> text hidden, only black bg visible
;     Row 4: "Blink Bright" attr 0x8F  -> text hidden, only black bg visible
;     Row 5: "Blink Red"    attr 0xC7  -> text hidden, only red bg visible
;     Cursor hidden. Blinking chars show background only.
;
;   Screen 3a — blink_and_cursor_04_cursor_visible.png
;     Row 10: "Cursor:" label, attr 0x07
;     Cursor visible at (12, 40), default underline shape.
;
;   Screen 3b — blink_and_cursor_05_cursor_blanked.png
;     Same VRAM as 3a, blink_phase=true.
;     Row 10: "Cursor:" label unchanged (no blink attr).
;     Cursor hidden (blanked phase of cursor blink cycle).

[CPU 8086]
org 0x100
BITS 16

start:
    ; ---- Screen 1: Intensity mode (blink OFF) ----
    mov ax, 0x0003
    int 0x10

    ; Disable blink (intensity mode): INT 10h AH=10h AL=03h BL=00h
    mov ax, 0x1003
    mov bl, 0x00
    int 0x10

    mov dh, 2
    mov dl, 2
    call set_cursor
    mov bl, 0x07
    mov si, str_normal
    call write_attr

    mov dh, 3
    mov dl, 2
    call set_cursor
    mov bl, 0x87
    mov si, str_bright_bg
    call write_attr

    mov dh, 4
    mov dl, 2
    call set_cursor
    mov bl, 0x8F
    mov si, str_bright_both
    call write_attr

    mov dh, 5
    mov dl, 2
    call set_cursor
    mov bl, 0xC7
    mov si, str_red_bg
    call write_attr

    ; Hide cursor
    mov dh, 25
    mov dl, 0
    call set_cursor
    call wait_key

    ; ---- Screen 2: Blink enabled ----
    mov ax, 0x0003
    int 0x10

    ; Enable blink: INT 10h AH=10h AL=03h BL=01h
    mov ax, 0x1003
    mov bl, 0x01
    int 0x10

    mov dh, 2
    mov dl, 2
    call set_cursor
    mov bl, 0x07
    mov si, str_no_blink
    call write_attr

    mov dh, 3
    mov dl, 2
    call set_cursor
    mov bl, 0x87
    mov si, str_blink
    call write_attr

    mov dh, 4
    mov dl, 2
    call set_cursor
    mov bl, 0x8F
    mov si, str_blink_bright
    call write_attr

    mov dh, 5
    mov dl, 2
    call set_cursor
    mov bl, 0xC7
    mov si, str_blink_red
    call write_attr

    ; Hide cursor
    mov dh, 25
    mov dl, 0
    call set_cursor
    ; Test captures two screenshots here (visible + blanked blink phase)
    call wait_key

    ; ---- Screen 3: Cursor test ----
    mov ax, 0x0003
    int 0x10

    mov dh, 10
    mov dl, 33
    call set_cursor
    mov bl, 0x07
    mov si, str_cursor
    call write_attr

    ; Place visible cursor at row 12, col 40
    mov dh, 12
    mov dl, 40
    call set_cursor
    call wait_key

    ; Exit
    mov ah, 0x4C
    xor al, al
    int 0x21

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

; write_attr - write null-terminated string with attribute at cursor
; SI = string, BL = attribute
write_attr:
    push ax
    push bx
    push cx
    push dx
    push si
.loop:
    lodsb
    cmp al, 0
    je .done
    mov ah, 0x09
    mov bh, 0x00
    mov cx, 1
    int 0x10
    ; advance cursor
    mov ah, 0x03
    mov bh, 0x00
    int 0x10
    inc dl
    call set_cursor
    jmp .loop
.done:
    pop si
    pop dx
    pop cx
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
    mov [es:0x1C], al
    sti
    pop ax
    pop es
    xor ah, ah
    int 0x16
    ret

; ============================================================
; Data
; ============================================================

str_normal       db "Normal", 0
str_bright_bg    db "Bright BG", 0
str_bright_both  db "Bright Both", 0
str_red_bg       db "Red BG", 0
str_no_blink     db "No Blink", 0
str_blink        db "Blink", 0
str_blink_bright db "Blink Bright", 0
str_blink_red    db "Blink Red", 0
str_cursor       db "Cursor:", 0
