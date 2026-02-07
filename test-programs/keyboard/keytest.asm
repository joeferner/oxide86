; keytest.asm - Keyboard test program
; Shows scan code and ASCII for each keypress

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
    shr al, 4
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
    shr al, 4
    call hexdigit

    ; ASCII low nibble
    mov al, bl
    and al, 0x0F
    call hexdigit

    ; Check for ESC
    cmp bl, 27
    jne .check_printable
    mov ah, 0x09
    mov dx, esc_msg
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

bye:
    db 13, 10, 'Done!', 13, 10, '$'
