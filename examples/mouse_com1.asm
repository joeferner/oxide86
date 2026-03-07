[CPU 8086]
org 0x0100          ; .COM file

; Mouse cursor demo using MS Serial Mouse on COM1.
; Move the mouse to move the '*' cursor around the screen.
; Press any key to exit.
;
; MS Mouse serial protocol (3-byte packets):
;   Byte 1: bit 6 set (sync), bit 5=LB, bit 4=RB, bits 3-2=Y7-Y6, bits 1-0=X7-X6
;   Byte 2: X delta lower 6 bits (6-bit signed, bits 7-6 come from byte 1)
;   Byte 3: Y delta lower 6 bits (6-bit signed, bits 7-6 come from byte 1)
;
; Initialization: INT 14h AH=00h with AL=0x82 (1200 baud, 7N1) raises DTR,
; which triggers the mouse to send its 'M' identification byte.

SCREEN_COLS equ 80
SCREEN_ROWS equ 25

start:
    ; Initialize COM1: 1200 baud, 7N1 (MS Serial Mouse settings).
    ; Raising DTR causes the mouse to send 'M' identification.
    mov ah, 0x00
    mov al, 0x82        ; 1200 baud, no parity, 1 stop bit, 7 data bits
    xor dx, dx          ; COM1
    int 0x14

    ; Drain the 'M' identification byte (blocking read, ignore result)
    mov ah, 0x02
    xor dx, dx
    int 0x14

    ; Hide the blinking hardware text cursor
    mov ah, 0x01
    mov cx, 0x2000      ; invisible cursor shape
    int 0x10

    ; Clear screen (scroll 0 lines = clear entire window)
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

    ; Check COM1 line status (AH bit 0 = data ready)
    mov ah, 0x03
    xor dx, dx
    int 0x14
    test ah, 0x01
    jz main_loop        ; no data, keep polling

    ; Read candidate sync byte
    mov ah, 0x02
    xor dx, dx
    int 0x14
    test ah, 0x80       ; timeout / error?
    jnz main_loop
    test al, 0x40       ; bit 6 must be set for a sync byte
    jz main_loop        ; not a sync byte, discard and retry

    mov [b1], al        ; save byte 1

    ; Wait for and read byte 2 (X delta)
.wait_b2:
    mov ah, 0x03
    xor dx, dx
    int 0x14
    test ah, 0x01
    jz .wait_b2
    mov ah, 0x02
    xor dx, dx
    int 0x14
    test ah, 0x80
    jnz main_loop
    mov [b2], al        ; save byte 2

    ; Wait for and read byte 3 (Y delta)
.wait_b3:
    mov ah, 0x03
    xor dx, dx
    int 0x14
    test ah, 0x01
    jz .wait_b3
    mov ah, 0x02
    xor dx, dx
    int 0x14
    test ah, 0x80
    jnz main_loop
    mov [b3], al        ; save byte 3

    ; Erase cursor at old position before moving
    call erase_cursor

    ; --- Compute signed X delta ---
    ; Full 8-bit value: (b1[1:0] << 6) | b2[5:0]
    mov al, [b1]
    and al, 0x03        ; keep bits 1-0 (X high bits)
    mov cl, 6
    shl al, cl          ; shift to bits 7-6
    mov ah, [b2]
    and ah, 0x3F        ; keep low 6 bits
    or al, ah           ; AL = 8-bit signed X delta
    cbw                 ; AX = sign-extended 16-bit X delta

    ; cur_col += X, clamped to [0, SCREEN_COLS-1]
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

    ; --- Compute signed Y delta ---
    ; Full 8-bit value: (b1[3:2] << 4) | b3[5:0]
    mov al, [b1]
    and al, 0x0C        ; keep bits 3-2 (Y high bits)
    mov cl, 4
    shl al, cl          ; shift to bits 7-6
    mov ah, [b3]
    and ah, 0x3F        ; keep low 6 bits
    or al, ah           ; AL = 8-bit signed Y delta
    cbw                 ; AX = sign-extended 16-bit Y delta

    ; cur_row += Y, clamped to [1, SCREEN_ROWS-1] (row 0 reserved for title)
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

    call draw_cursor
    call update_buttons
    jmp main_loop

exit:
    ; Consume the keypress so it doesn't echo to the shell
    mov ah, 0x00
    int 0x16

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
; Reads b1: bit 5 = left button, bit 4 = right button
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

    mov al, [b1]
    test al, 0x20       ; bit 5 = left button
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

    mov al, [b1]
    test al, 0x10       ; bit 4 = right button
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
title_msg:   db 'Mouse demo - move cursor with mouse, press any key to exit$'
lb_label:    db 'L:$'
rb_label:    db ' R:$'
btn_on_str:  db 'ON  $'
btn_off_str: db 'OFF $'
cur_col:   db SCREEN_COLS / 2
cur_row:   db SCREEN_ROWS / 2
b1:        db 0
b2:        db 0
b3:        db 0
