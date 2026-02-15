; joystick_test.com - Test IBM Game Control Adapter (port 0x201)
; Reads joystick port and displays axis timer states and button states
; Press any key to exit

org 0x100

section .text

start:
    ; Set up video mode (80x25 color text)
    mov ax, 0x0003
    int 0x10

    ; Display header
    mov ah, 0x13          ; Write string
    mov al, 0x01          ; Update cursor
    mov bh, 0             ; Page 0
    mov bl, 0x0F          ; White on black
    mov cx, header_len
    mov dx, 0x0000        ; Row 0, col 0
    push cs
    pop es
    mov bp, header
    int 0x10

    ; Display instructions
    mov ah, 0x13
    mov al, 0x01
    mov bh, 0
    mov bl, 0x07          ; Gray on black
    mov cx, inst_len
    mov dx, 0x0200        ; Row 2, col 0
    mov bp, instructions
    int 0x10

main_loop:
    ; Check for keystroke (non-blocking)
    mov ah, 0x01
    int 0x16
    jnz exit              ; Exit if key pressed

    ; Fire joystick one-shots (write any value to 0x201)
    mov al, 0xFF
    mov dx, 0x201
    out dx, al

    ; Small delay to let timers run
    mov cx, 500
delay_loop:
    loop delay_loop

    ; Read joystick port
    mov dx, 0x201
    in al, dx
    mov [port_value], al

    ; Display port value at row 4
    call display_port_value

    ; Display axis timer states (bits 0-3)
    mov bl, [port_value]
    call display_axis_timers

    ; Display button states (bits 4-7)
    mov bl, [port_value]
    call display_buttons

    ; Small delay before next read
    mov cx, 10000
delay_loop2:
    loop delay_loop2

    jmp main_loop

exit:
    ; Clear keyboard buffer
    mov ah, 0x00
    int 0x16

    ; Restore video mode and exit
    mov ax, 0x0003
    int 0x10
    mov ax, 0x4C00
    int 0x21

; Display the raw port value as two hex digits
display_port_value:
    mov ah, 0x13
    mov al, 0x01
    mov bh, 0
    mov bl, 0x0E          ; Yellow on black
    mov cx, port_label_len
    mov dx, 0x0400        ; Row 4, col 0
    mov bp, port_label
    int 0x10

    ; Convert high nibble to hex
    mov al, [port_value]
    shr al, 4
    call hex_to_ascii
    mov [hex_output], al

    ; Convert low nibble to hex
    mov al, [port_value]
    and al, 0x0F
    call hex_to_ascii
    mov [hex_output+1], al

    ; Display hex value
    mov ah, 0x13
    mov al, 0x00          ; Don't update cursor
    mov bh, 0
    mov bl, 0x0F
    mov cx, 2
    mov dx, 0x040E        ; Row 4, col 14
    mov bp, hex_output
    int 0x10

    ret

; Display axis timer states
display_axis_timers:
    mov ah, 0x13
    mov al, 0x01
    mov bh, 0
    mov bl, 0x0B          ; Cyan on black
    mov cx, axis_label_len
    mov dx, 0x0600        ; Row 6, col 0
    mov bp, axis_label
    int 0x10

    ; Joystick A X-axis (bit 0)
    mov ah, 0x13
    mov al, 0x00
    mov bh, 0
    mov bl, 0x0F
    mov cx, axis_ax_len
    mov dx, 0x0700
    mov bp, axis_ax_label
    int 0x10
    mov al, [port_value]
    test al, 0x01
    call display_timer_state
    mov dx, 0x0710
    call display_status

    ; Joystick A Y-axis (bit 1)
    mov ah, 0x13
    mov al, 0x00
    mov bh, 0
    mov bl, 0x0F
    mov cx, axis_ay_len
    mov dx, 0x0800
    mov bp, axis_ay_label
    int 0x10
    mov al, [port_value]
    test al, 0x02
    call display_timer_state
    mov dx, 0x0810
    call display_status

    ; Joystick B X-axis (bit 2)
    mov ah, 0x13
    mov al, 0x00
    mov bh, 0
    mov bl, 0x0F
    mov cx, axis_bx_len
    mov dx, 0x0900
    mov bp, axis_bx_label
    int 0x10
    mov al, [port_value]
    test al, 0x04
    call display_timer_state
    mov dx, 0x0910
    call display_status

    ; Joystick B Y-axis (bit 3)
    mov ah, 0x13
    mov al, 0x00
    mov bh, 0
    mov bl, 0x0F
    mov cx, axis_by_len
    mov dx, 0x0A00
    mov bp, axis_by_label
    int 0x10
    mov al, [port_value]
    test al, 0x08
    call display_timer_state
    mov dx, 0x0A10
    call display_status

    ret

; Display button states
display_buttons:
    mov ah, 0x13
    mov al, 0x01
    mov bh, 0
    mov bl, 0x0D          ; Magenta on black
    mov cx, btn_label_len
    mov dx, 0x0C00        ; Row 12, col 0
    mov bp, btn_label
    int 0x10

    ; Joystick A Button 1 (bit 4, inverted: 0=pressed)
    mov ah, 0x13
    mov al, 0x00
    mov bh, 0
    mov bl, 0x0F
    mov cx, btn_a1_len
    mov dx, 0x0D00
    mov bp, btn_a1_label
    int 0x10
    mov al, [port_value]
    test al, 0x10
    call display_button_state
    mov dx, 0x0D10
    call display_status

    ; Joystick A Button 2 (bit 5, inverted)
    mov ah, 0x13
    mov al, 0x00
    mov bh, 0
    mov bl, 0x0F
    mov cx, btn_a2_len
    mov dx, 0x0E00
    mov bp, btn_a2_label
    int 0x10
    mov al, [port_value]
    test al, 0x20
    call display_button_state
    mov dx, 0x0E10
    call display_status

    ; Joystick B Button 1 (bit 6, inverted)
    mov ah, 0x13
    mov al, 0x00
    mov bh, 0
    mov bl, 0x0F
    mov cx, btn_b1_len
    mov dx, 0x0F00
    mov bp, btn_b1_label
    int 0x10
    mov al, [port_value]
    test al, 0x40
    call display_button_state
    mov dx, 0x0F10
    call display_status

    ; Joystick B Button 2 (bit 7, inverted)
    mov ah, 0x13
    mov al, 0x00
    mov bh, 0
    mov bl, 0x0F
    mov cx, btn_b2_len
    mov dx, 0x1000
    mov bp, btn_b2_label
    int 0x10
    mov al, [port_value]
    test al, 0x80
    call display_button_state
    mov dx, 0x1010
    call display_status

    ret

; Set status string based on timer state (ZF set = timed out, ZF clear = running)
display_timer_state:
    jnz .running
    mov bp, timed_out
    ret
.running:
    mov bp, running
    ret

; Set status string based on button state (ZF set = pressed, ZF clear = released)
; Note: button bits are inverted (0=pressed, 1=released)
display_button_state:
    jz .pressed
    mov bp, released
    ret
.pressed:
    mov bp, pressed
    ret

; Display status string (BP points to string)
display_status:
    mov ah, 0x13
    mov al, 0x00
    mov bh, 0
    mov bl, 0x0A          ; Green on black
    mov cx, 8
    int 0x10
    ret

; Convert hex nibble (0-15) in AL to ASCII character
hex_to_ascii:
    cmp al, 10
    jb .digit
    add al, 'A' - 10
    ret
.digit:
    add al, '0'
    ret

section .data

header:         db 'IBM Game Control Adapter (Port 0x201) Test'
header_len:     equ $ - header

instructions:   db 'Press any key to exit'
inst_len:       equ $ - instructions

port_label:     db 'Port value: '
port_label_len: equ $ - port_label

axis_label:     db 'Axis Timers:'
axis_label_len: equ $ - axis_label

axis_ax_label:  db 'Joy A X-axis'
axis_ax_len:    equ $ - axis_ax_label

axis_ay_label:  db 'Joy A Y-axis'
axis_ay_len:    equ $ - axis_ay_label

axis_bx_label:  db 'Joy B X-axis'
axis_bx_len:    equ $ - axis_bx_label

axis_by_label:  db 'Joy B Y-axis'
axis_by_len:    equ $ - axis_by_label

btn_label:      db 'Buttons:'
btn_label_len:  equ $ - btn_label

btn_a1_label:   db 'Joy A Btn 1 '
btn_a1_len:     equ $ - btn_a1_label

btn_a2_label:   db 'Joy A Btn 2 '
btn_a2_len:     equ $ - btn_a2_label

btn_b1_label:   db 'Joy B Btn 1 '
btn_b1_len:     equ $ - btn_b1_label

btn_b2_label:   db 'Joy B Btn 2 '
btn_b2_len:     equ $ - btn_b2_label

timed_out:      db 'TimedOut'
running:        db 'Running '
pressed:        db 'Pressed '
released:       db 'Released'

section .bss

port_value:     resb 1
hex_output:     resb 2
