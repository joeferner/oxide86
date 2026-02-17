; Test INT 10h/AH=11h/AL=30h - Get Font Information
; This program tests all font pointer types and displays the results

[CPU 8086]
org 0x100

start:
    ; Display header
    mov dx, msg_header
    call print_string

    ; Test pointer type 0x00 (INT 1Fh - 8x8 graphics)
    mov bh, 0x00
    call test_font_info
    mov dx, msg_type00
    call print_result

    ; Test pointer type 0x03 (ROM 8x8)
    mov bh, 0x03
    call test_font_info
    mov dx, msg_type03
    call print_result

    ; Test pointer type 0x06 (ROM 8x16)
    mov bh, 0x06
    call test_font_info
    mov dx, msg_type06
    call print_result

    ; Exit
    mov ax, 0x4C00
    int 0x21

; Test font info for pointer type in BH
; Returns: ES:BP = font pointer, CX = bytes per char, DL = rows-1
test_font_info:
    push bx
    mov ax, 0x1130      ; AH=11h, AL=30h
    int 0x10
    pop bx
    ret

; Print result of font query
; Input: ES:BP = font pointer, CX = bytes per char, DL = rows-1
;        DX = message pointer
print_result:
    push ax
    push bx
    push cx
    push dx
    push es
    push bp

    ; Print message
    call print_string

    ; Print "ES:BP = "
    mov dx, msg_espp
    call print_string

    ; Print ES (segment)
    mov ax, es
    call print_hex16
    mov al, ':'
    call print_char

    ; Print BP (offset)
    mov ax, bp
    call print_hex16

    ; Print " CX = "
    mov dx, msg_cx
    call print_string

    ; Print CX (bytes per char)
    mov ax, cx
    call print_hex16

    ; Print " DL = "
    mov dx, msg_dl
    call print_string

    ; Print DL (rows-1)
    mov al, dl
    xor ah, ah
    call print_hex16

    ; Print newline
    mov dx, msg_newline
    call print_string

    pop bp
    pop es
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Print string at DS:DX
print_string:
    push ax
    mov ah, 0x09
    int 0x21
    pop ax
    ret

; Print character in AL
print_char:
    push ax
    push dx
    mov dl, al
    mov ah, 0x02
    int 0x21
    pop dx
    pop ax
    ret

; Print 16-bit hex value in AX
print_hex16:
    push ax
    push bx
    push cx
    push dx

    mov cx, 4           ; 4 hex digits
    mov bx, ax          ; Save value

.loop:
    mov cl, 4
    rol bx, cl           ; Rotate left by 4 bits
    mov al, bl
    and al, 0x0F        ; Get low nibble
    add al, '0'
    cmp al, '9'
    jbe .print
    add al, 7           ; A-F

.print:
    mov dl, al
    mov ah, 0x02
    int 0x21
    loop .loop

    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Data
msg_header:     db 'Testing INT 10h/AH=11h/AL=30h - Get Font Information', 13, 10, '$'
msg_type00:     db 'Type 00h (INT 1Fh - 8x8 graphics): $'
msg_type03:     db 'Type 03h (ROM 8x8 double-dot):     $'
msg_type06:     db 'Type 06h (ROM 8x16):               $'
msg_espp:       db 'ES:BP=$'
msg_cx:         db ' CX=$'
msg_dl:         db ' DL=$'
msg_newline:    db 13, 10, '$'
