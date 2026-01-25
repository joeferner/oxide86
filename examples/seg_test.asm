; Test segment override
org 0x100

section .text
start:
    ; Set DS to 0x0040
    mov ax, 0x0040
    mov ds, ax

    ; Write via DS (no override)
    mov word [0x10], 0x5678

    ; Read it back
    mov ax, [0x10]
    mov dx, ax
    call print_hex_word
    call print_newline

    ; Now set ES to 0x0040 too
    mov ax, 0x0040
    mov es, ax

    ; Read via ES override
    mov ax, [es:0x10]
    mov dx, ax
    call print_hex_word
    call print_newline

    ; Exit
    mov ax, 0x4C00
    int 0x21

print_hex_word:
    push ax
    push dx
    mov ax, dx
    push ax
    mov al, ah
    call print_hex_byte
    pop ax
    call print_hex_byte
    pop dx
    pop ax
    ret

print_hex_byte:
    push ax
    shr al, 4
    call print_hex_nibble
    pop ax
    and al, 0x0F
    call print_hex_nibble
    ret

print_hex_nibble:
    push ax
    push dx
    cmp al, 10
    jl .digit
    add al, 'A' - 10
    jmp .print
.digit:
    add al, '0'
.print:
    mov dl, al
    mov ah, 0x02
    int 0x21
    pop dx
    pop ax
    ret

print_newline:
    push ax
    push dx
    mov dl, 0x0D
    mov ah, 0x02
    int 0x21
    mov dl, 0x0A
    mov ah, 0x02
    int 0x21
    pop dx
    pop ax
    ret
