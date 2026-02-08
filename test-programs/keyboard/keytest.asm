; keytest.asm - Keyboard test program
; Shows scan code and ASCII for each keypress

[CPU 8086]
org 0x0100

start:
    ; Print banner
    mov ah, 0x09
    mov dx, banner
    int 0x21

main_loop:
    ; Read key
    mov ah, 0x00
    int 0x16

    ; Save: BH=scan, BL=ASCII
    mov bx, ax

    ; Newline
    mov ah, 0x02
    mov dl, 13
    int 0x21
    mov dl, 10
    int 0x21

    ; "S:"
    mov dl, 'S'
    int 0x21
    mov dl, ':'
    int 0x21

    ; Scan high nibble
    mov al, bh
    mov cl, 4
    shr al, cl
    call hexdigit

    ; Scan low nibble
    mov al, bh
    and al, 0x0F
    call hexdigit

    ; " A:"
    mov ah, 0x02
    mov dl, ' '
    int 0x21
    mov dl, 'A'
    int 0x21
    mov dl, ':'
    int 0x21

    ; ASCII high nibble
    mov al, bl
    mov cl, 4
    shr al, cl
    call hexdigit

    ; ASCII low nibble
    mov al, bl
    and al, 0x0F
    call hexdigit

    ; Check for ESC
    cmp bl, 27
    jne .check_backspace
    mov ah, 0x09
    mov dx, esc_msg
    int 0x21
    jmp main_loop

.check_backspace:
    ; Check for Backspace
    cmp bl, 8
    jne .check_enter
    mov ah, 0x09
    mov dx, backspace_msg
    int 0x21
    jmp main_loop

.check_enter:
    ; Check for Enter
    cmp bl, 13
    jne .check_arrows
    mov ah, 0x09
    mov dx, enter_msg
    int 0x21
    jmp main_loop

.check_arrows:
    ; Arrow keys have ASCII 0 (extended keys), check scan code
    cmp bl, 0
    jne .check_ctrl

    ; Up arrow
    cmp bh, 0x48
    jne .not_up
    mov ah, 0x09
    mov dx, up_msg
    int 0x21
    jmp main_loop

.not_up:
    ; Down arrow
    cmp bh, 0x50
    jne .not_down
    mov ah, 0x09
    mov dx, down_msg
    int 0x21
    jmp main_loop

.not_down:
    ; Left arrow
    cmp bh, 0x4B
    jne .not_left
    mov ah, 0x09
    mov dx, left_msg
    int 0x21
    jmp main_loop

.not_left:
    ; Right arrow
    cmp bh, 0x4D
    jne .check_ctrl
    mov ah, 0x09
    mov dx, right_msg
    int 0x21
    jmp main_loop

.check_ctrl:
    ; Check for Ctrl+letter (ASCII 1-26 = Ctrl+A through Ctrl+Z)
    cmp bl, 1
    jb .check_printable
    cmp bl, 26
    ja .check_printable

    ; Check for Ctrl+C (ASCII 3) - exit
    cmp bl, 3
    je exit_prog

    ; Display "Ctrl+"
    mov ah, 0x02
    mov dl, ' '
    int 0x21
    mov ah, 0x09
    mov dx, ctrl_msg
    int 0x21

    ; Convert ASCII to letter (add 'A'-1 to get the letter)
    mov ah, 0x02
    mov dl, bl
    add dl, 'A' - 1
    int 0x21

    jmp main_loop

.check_printable:
    ; Show char if printable
    cmp bl, 32
    jb main_loop
    cmp bl, 126
    ja main_loop

    mov ah, 0x02
    mov dl, ' '
    int 0x21
    mov dl, '['
    int 0x21
    mov dl, bl
    int 0x21
    mov dl, ']'
    int 0x21

    jmp main_loop

exit_prog:
    mov ah, 0x09
    mov dx, bye
    int 0x21
    mov ax, 0x4C00
    int 0x21

hexdigit:
    cmp al, 9
    jbe .num
    add al, 'A'-10
    jmp .out
.num:
    add al, '0'
.out:
    mov dl, al
    mov ah, 0x02
    int 0x21
    ret

banner:
    db '=== Keyboard Test ===', 13, 10
    db 'Format: S:XX A:YY [char]', 13, 10
    db 'S=Scan code, A=ASCII', 13, 10
    db 'Try: arrows, ESC, Ctrl+keys, F-keys', 13, 10, '$'

esc_msg:
    db ' ESC', '$'

backspace_msg:
    db ' BACKSPACE', '$'

enter_msg:
    db ' ENTER', '$'

up_msg:
    db ' UP', '$'

down_msg:
    db ' DOWN', '$'

left_msg:
    db ' LEFT', '$'

right_msg:
    db ' RIGHT', '$'

ctrl_msg:
    db 'Ctrl+', '$'

bye:
    db 13, 10, 'Done!', 13, 10, '$'
