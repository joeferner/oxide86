[CPU 8086]
org 0x0100          ; .COM file

; Mouse cursor demo using PS/2 mouse via INT 15h AH=C2h BIOS services.
; Move the mouse to move the '*' cursor around the screen.
; Press any key to exit.
;
; Run with: oxide86 --ps2-mouse mouse_ps2.com
;
; Initialization sequence (INT 15h AH=C2h):
;   AL=05h BH=03h  — initialize, 3-byte packets
;   AL=00h BH=01h  — enable mouse
;   AL=07h ES:BX   — register far-call callback handler
;
; Callback receives (via INT 74h → FAR CALL):
;   AL = status byte: bit 0=left button, bit 1=right button,
;                     bit 4=X sign, bit 5=Y sign
;   BL = X delta (signed byte)
;   CL = Y delta (signed byte)
;   DL = Z delta (0 for standard mouse)
; Must return with RETF.

SCREEN_COLS equ 80
SCREEN_ROWS equ 25

start:
    ; Initialize PS/2 mouse: 3-byte packets (AL=05h, BH=03h)
    mov ax, 0xC205
    mov bh, 3
    int 0x15
    jc init_fail

    ; Enable PS/2 mouse (AL=00h, BH=01h)
    mov ax, 0xC200
    mov bh, 1
    int 0x15
    jc init_fail

    ; Register callback handler (AL=07h, ES:BX = far pointer, CX = event mask)
    ; CX bits: 0=Y movement, 1=X movement, 2=Y sign, 3=X sign,
    ;          4=right button, 5=middle button, 6=left button
    mov ax, 0xC207
    push cs
    pop es
    mov bx, mouse_handler
    mov cx, 0x07        ; enable all standard events
    int 0x15
    jc init_fail

    ; Hide the blinking hardware text cursor
    mov ah, 0x01
    mov cx, 0x2000      ; invisible cursor shape
    int 0x10

    ; Clear screen
    mov ah, 0x06
    xor al, al
    mov bh, 0x07        ; attribute: light grey on black
    xor cx, cx          ; top-left corner (row 0, col 0)
    mov dx, 0x184F      ; bottom-right corner (row 24, col 79)
    int 0x10

    ; Print title at top of screen
    mov ah, 0x02        ; set cursor position
    xor bx, bx          ; page 0
    xor dx, dx          ; row 0, col 0
    int 0x10
    mov ah, 0x09        ; DOS print string
    mov dx, title_msg
    int 0x21

    ; Draw initial cursor at centre of screen
    call draw_cursor
    call update_buttons

main_loop:
    ; Non-blocking keyboard check: ZF=0 if a key is waiting
    mov ah, 0x01
    int 0x16
    jnz exit

    ; Check if callback signalled new data
    cmp byte [mouse_ready], 0
    je main_loop

    ; Erase cursor at old position before moving
    call erase_cursor

    ; --- Apply X delta ---
    mov al, [mouse_dx]
    cbw                 ; sign-extend AL → AX
    mov bl, [cur_col]
    xor bh, bh
    add bx, ax
    cmp bx, 0
    jge .cx_lo_ok
    xor bx, bx
.cx_lo_ok:
    cmp bx, SCREEN_COLS - 1
    jle .cx_hi_ok
    mov bx, SCREEN_COLS - 1
.cx_hi_ok:
    mov [cur_col], bl

    ; --- Apply Y delta ---
    mov al, [mouse_dy]
    cbw                 ; sign-extend AL → AX
    mov bl, [cur_row]
    xor bh, bh
    add bx, ax
    cmp bx, 1
    jge .cy_lo_ok
    mov bx, 1
.cy_lo_ok:
    cmp bx, SCREEN_ROWS - 1
    jle .cy_hi_ok
    mov bx, SCREEN_ROWS - 1
.cy_hi_ok:
    mov [cur_row], bl

    mov byte [mouse_ready], 0   ; clear flag

    call draw_cursor
    call update_buttons
    jmp main_loop

init_fail:
    mov ah, 0x09
    mov dx, fail_msg
    int 0x21
    mov ah, 0x4C
    mov al, 1
    int 0x21

exit:
    ; Consume the keypress so it doesn't echo to the shell
    mov ah, 0x00
    int 0x16

    ; Disable PS/2 mouse (AL=00h, BH=00h)
    mov ax, 0xC200
    mov bh, 0
    int 0x15

    ; Restore blinking hardware cursor
    mov ah, 0x01
    mov cx, 0x0607      ; standard underscore cursor
    int 0x10

    ; Clear screen
    mov ah, 0x06
    xor al, al
    mov bh, 0x07
    xor cx, cx
    mov dx, 0x184F
    int 0x10

    ; Move hardware cursor to top-left
    mov ah, 0x02
    xor bx, bx
    xor dx, dx
    int 0x10

    mov ah, 0x4C
    xor al, al
    int 0x21

; -----------------------------------------------------------------------
; mouse_handler: PS/2 mouse callback (FAR CALL from INT 74h BIOS handler)
;   AL = status byte (bit 0=left, bit 1=right)
;   BL = X delta (signed byte)
;   CL = Y delta (signed byte)
; -----------------------------------------------------------------------
mouse_handler:
    mov [mouse_status], al
    mov [mouse_dx], bl
    mov [mouse_dy], cl
    mov byte [mouse_ready], 1
    retf

; -----------------------------------------------------------------------
; draw_cursor: write '*' at (cur_row, cur_col) in bright white
; -----------------------------------------------------------------------
draw_cursor:
    mov ah, 0x02        ; set cursor position
    xor bh, bh          ; page 0
    mov dh, [cur_row]
    mov dl, [cur_col]
    int 0x10
    mov ah, 0x09        ; write character + attribute at cursor
    mov al, '*'
    xor bh, bh
    mov bl, 0x0F        ; bright white on black
    mov cx, 1
    int 0x10
    ret

; -----------------------------------------------------------------------
; erase_cursor: overwrite '*' with a space at (cur_row, cur_col)
; -----------------------------------------------------------------------
erase_cursor:
    mov ah, 0x02
    xor bh, bh
    mov dh, [cur_row]
    mov dl, [cur_col]
    int 0x10
    mov ah, 0x09
    mov al, ' '
    xor bh, bh
    mov bl, 0x07        ; normal attribute
    mov cx, 1
    int 0x10
    ret

; -----------------------------------------------------------------------
; update_buttons: display left/right button state at row 0, col 65
; Reads mouse_status: bit 0 = left button, bit 1 = right button
; -----------------------------------------------------------------------
update_buttons:
    mov ah, 0x02        ; set cursor position
    xor bh, bh
    mov dh, 0           ; row 0
    mov dl, 65          ; col 65
    int 0x10

    mov ah, 0x09
    mov dx, lb_label    ; "L:"
    int 0x21

    mov al, [mouse_status]
    test al, 0x01       ; bit 0 = left button
    jz .lb_off
    mov dx, btn_on_str
    jmp .print_lb
.lb_off:
    mov dx, btn_off_str
.print_lb:
    mov ah, 0x09
    int 0x21

    mov ah, 0x09
    mov dx, rb_label    ; " R:"
    int 0x21

    mov al, [mouse_status]
    test al, 0x02       ; bit 1 = right button
    jz .rb_off
    mov dx, btn_on_str
    jmp .print_rb
.rb_off:
    mov dx, btn_off_str
.print_rb:
    mov ah, 0x09
    int 0x21
    ret

; -----------------------------------------------------------------------
; Data
; -----------------------------------------------------------------------
title_msg:    db 'PS/2 Mouse demo - move cursor with mouse, press any key to exit$'
fail_msg:     db 'PS/2 mouse init failed$'
lb_label:     db 'L:$'
rb_label:     db ' R:$'
btn_on_str:   db 'ON  $'
btn_off_str:  db 'OFF $'
cur_col:      db SCREEN_COLS / 2
cur_row:      db SCREEN_ROWS / 2
mouse_status: db 0
mouse_dx:     db 0
mouse_dy:     db 0
mouse_ready:  db 0
