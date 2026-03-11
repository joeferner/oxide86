; Game port demo.
; Reads joystick axis positions and button states and displays them live.
; Exits when any key is pressed.
;
; Game port I/O (port 0x201):
;   Write: fire one-shot timing circuit for all four axes.
;   Read bit 0: Joystick 1 X-axis one-shot (1 = timing, 0 = done)
;   Read bit 1: Joystick 1 Y-axis one-shot (1 = timing, 0 = done)
;   Read bit 2: Joystick 2 X-axis one-shot (1 = timing, 0 = done)
;   Read bit 3: Joystick 2 Y-axis one-shot (1 = timing, 0 = done)
;   Read bit 4: Button 1 (0 = pressed, 1 = released)
;   Read bit 5: Button 2 (0 = pressed, 1 = released)
;   Read bit 6: Button 3 (0 = pressed, 1 = released)
;   Read bit 7: Button 4 (0 = pressed, 1 = released)
;
; Axis reading technique:
;   Write to 0x201, then count loop iterations until the timing bit clears.
;   Higher counts = higher resistance = joystick pushed further from center.

[CPU 8086]
org 0x0100              ; .COM file

GAME_PORT equ 0x201

start:
    ; Hide blinking cursor
    mov ah, 0x01
    mov cx, 0x2000
    int 0x10

    ; Clear screen
    mov ah, 0x06
    xor al, al
    mov bh, 0x07
    xor cx, cx
    mov dx, 0x184F
    int 0x10

    ; Print title at row 0
    mov ah, 0x02
    xor bx, bx
    xor dx, dx
    int 0x10
    mov ah, 0x09
    mov dx, title_msg
    int 0x21

main_loop:
    ; Read current button state (bits 4-7; no one-shot needed)
    mov dx, GAME_PORT
    in al, dx
    mov [port_val], al

    ; Read each axis: fire one-shot, count until the bit clears
    call read_x1
    mov [x1_count], cx

    call read_y1
    mov [y1_count], cx

    call read_x2
    mov [x2_count], cx

    call read_y2
    mov [y2_count], cx

    ; Display joystick 1 axes at row 2
    mov ah, 0x02
    xor bx, bx
    mov dx, 0x0200
    int 0x10
    mov ah, 0x09
    mov dx, lbl_x1
    int 0x21
    mov ax, [x1_count]
    call print_dec
    mov ah, 0x09
    mov dx, lbl_y1
    int 0x21
    mov ax, [y1_count]
    call print_dec

    ; Display joystick 2 axes at row 3
    mov ah, 0x02
    xor bx, bx
    mov dx, 0x0300
    int 0x10
    mov ah, 0x09
    mov dx, lbl_x2
    int 0x21
    mov ax, [x2_count]
    call print_dec
    mov ah, 0x09
    mov dx, lbl_y2
    int 0x21
    mov ax, [y2_count]
    call print_dec

    ; Display button states at row 5
    mov ah, 0x02
    xor bx, bx
    mov dx, 0x0500
    int 0x10
    call show_buttons

    ; Exit when any key is pressed
    call check_exit

    jmp main_loop

exit:
    ; Consume the key from the buffer
    mov ah, 0x00
    int 0x16

    ; Restore cursor
    mov ah, 0x01
    mov cx, 0x0607
    int 0x10

    ; Clear screen
    mov ah, 0x06
    xor al, al
    mov bh, 0x07
    xor cx, cx
    mov dx, 0x184F
    int 0x10

    ; Home cursor
    mov ah, 0x02
    xor bx, bx
    xor dx, dx
    int 0x10

    mov ah, 0x4C
    xor al, al
    int 0x21

; -----------------------------------------------------------------------
; read_x1/y1/x2/y2: fire one-shot, count loops until the axis bit clears.
; Returns count in CX. Clobbers AX, DX. Count saturates at 0xFFFF.
; -----------------------------------------------------------------------
read_x1:
    mov dx, GAME_PORT
    out dx, al          ; any write fires the one-shot
    xor cx, cx
.loop:
    in al, dx
    test al, 0x01       ; bit 0 = joystick 1 X-axis
    jz .done
    inc cx
    jnz .loop
.done:
    ret

read_y1:
    mov dx, GAME_PORT
    out dx, al
    xor cx, cx
.loop:
    in al, dx
    test al, 0x02       ; bit 1 = joystick 1 Y-axis
    jz .done
    inc cx
    jnz .loop
.done:
    ret

read_x2:
    mov dx, GAME_PORT
    out dx, al
    xor cx, cx
.loop:
    in al, dx
    test al, 0x04       ; bit 2 = joystick 2 X-axis
    jz .done
    inc cx
    jnz .loop
.done:
    ret

read_y2:
    mov dx, GAME_PORT
    out dx, al
    xor cx, cx
.loop:
    in al, dx
    test al, 0x08       ; bit 3 = joystick 2 Y-axis
    jz .done
    inc cx
    jnz .loop
.done:
    ret

; -----------------------------------------------------------------------
; show_buttons: print B1..B4 ON/OFF from [port_val] at current cursor.
; Bits 4-7: 0 = pressed, 1 = released.
; -----------------------------------------------------------------------
show_buttons:
    mov ah, 0x09
    mov dx, lbl_b1
    int 0x21
    mov al, [port_val]
    test al, 0x10
    jz .b1_on
    mov dx, s_off
    jmp .pr_b1
.b1_on:
    mov dx, s_on
.pr_b1:
    mov ah, 0x09
    int 0x21

    mov ah, 0x09
    mov dx, lbl_b2
    int 0x21
    mov al, [port_val]
    test al, 0x20
    jz .b2_on
    mov dx, s_off
    jmp .pr_b2
.b2_on:
    mov dx, s_on
.pr_b2:
    mov ah, 0x09
    int 0x21

    mov ah, 0x09
    mov dx, lbl_b3
    int 0x21
    mov al, [port_val]
    test al, 0x40
    jz .b3_on
    mov dx, s_off
    jmp .pr_b3
.b3_on:
    mov dx, s_on
.pr_b3:
    mov ah, 0x09
    int 0x21

    mov ah, 0x09
    mov dx, lbl_b4
    int 0x21
    mov al, [port_val]
    test al, 0x80
    jz .b4_on
    mov dx, s_off
    jmp .pr_b4
.b4_on:
    mov dx, s_on
.pr_b4:
    mov ah, 0x09
    int 0x21

    ret

; -----------------------------------------------------------------------
; check_exit: check if a key is waiting; if so jump to exit.
; Uses INT 16h AH=01h (peek, non-blocking). ZF=0 means key available.
; -----------------------------------------------------------------------
check_exit:
    mov ah, 0x01
    int 0x16
    jnz exit
    ret

; -----------------------------------------------------------------------
; print_dec: print AX as a 5-digit decimal number. Clobbers AX, BX, DX.
; -----------------------------------------------------------------------
print_dec:
    xor dx, dx
    mov bx, 10000
    div bx              ; AX = ten-thousands digit, DX = remainder
    add al, '0'
    mov [dec_buf+0], al
    mov ax, dx

    xor dx, dx
    mov bx, 1000
    div bx
    add al, '0'
    mov [dec_buf+1], al
    mov ax, dx

    xor dx, dx
    mov bx, 100
    div bx
    add al, '0'
    mov [dec_buf+2], al
    mov ax, dx

    xor dx, dx
    mov bx, 10
    div bx
    add al, '0'
    mov [dec_buf+3], al
    add dl, '0'
    mov [dec_buf+4], dl

    mov ah, 0x09
    mov dx, dec_buf
    int 0x21
    ret

; -----------------------------------------------------------------------
; Data
; -----------------------------------------------------------------------
title_msg  db 'Game port demo - press any key to exit', 13, 10, '$'
lbl_x1     db 'J1 X: $'
lbl_y1     db '    Y: $'
lbl_x2     db 'J2 X: $'
lbl_y2     db '    Y: $'
lbl_b1     db 'B1: $'
lbl_b2     db '   B2: $'
lbl_b3     db '   B3: $'
lbl_b4     db '   B4: $'
s_on       db 'ON  $'
s_off      db 'OFF $'
dec_buf    db '00000$'

x1_count   dw 0
y1_count   dw 0
x2_count   dw 0
y2_count   dw 0
port_val   db 0
