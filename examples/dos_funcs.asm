; Test DOS system functions (INT 21h)
; Tests: Get/Set interrupt vector, Get DOS version, Get current drive

org 0x100

start:
    ; Test 1: Get DOS version (AH=30h)
    mov ah, 0x30
    int 0x21
    ; AL = major version, AH = minor version
    ; Expected: AL=3, AH=30 (DOS 3.30)

    ; Print DOS version
    push ax
    mov dx, msg_version
    mov ah, 0x09
    int 0x21
    pop ax

    ; Print major version
    push ax
    and al, 0x0F
    add al, '0'
    mov dl, al
    mov ah, 0x02
    int 0x21
    pop ax

    ; Print dot
    push ax
    mov dl, '.'
    mov ah, 0x02
    int 0x21
    pop ax

    ; Print minor version (tens digit)
    push ax
    mov al, ah
    xor ah, ah
    mov bl, 10
    div bl
    add al, '0'
    mov dl, al
    mov ah, 0x02
    int 0x21
    pop ax

    ; Print minor version (ones digit)
    push ax
    mov al, ah
    xor ah, ah
    mov bl, 10
    div bl
    add ah, '0'
    mov dl, ah
    mov ah, 0x02
    int 0x21
    pop ax

    ; Print newline
    mov dx, msg_newline
    mov ah, 0x09
    int 0x21

    ; Test 2: Get current drive (AH=19h)
    mov ah, 0x19
    int 0x21
    ; AL = current drive (0=A, 1=B, etc.)

    ; Print drive
    push ax
    mov dx, msg_drive
    mov ah, 0x09
    int 0x21
    pop ax

    add al, 'A'
    mov dl, al
    mov ah, 0x02
    int 0x21

    mov dx, msg_newline
    mov ah, 0x09
    int 0x21

    ; Test 3: Get interrupt vector (AH=35h, AL=21h for INT 21h)
    mov ah, 0x35
    mov al, 0x21
    int 0x21
    ; ES:BX = interrupt handler address

    ; Save the vector
    push es
    push bx

    ; Print message
    mov dx, msg_vector
    mov ah, 0x09
    int 0x21

    ; Print segment (ES)
    pop bx
    pop ax  ; ES in AX now
    push ax
    push bx

    call print_hex_word

    ; Print colon
    mov dl, ':'
    mov ah, 0x02
    int 0x21

    ; Print offset (BX)
    pop ax  ; BX in AX now
    call print_hex_word

    mov dx, msg_newline
    mov ah, 0x09
    int 0x21

    ; Test 4: Set interrupt vector (AH=25h)
    ; Save old vector first
    pop es  ; Restore ES

    ; Set up a new vector at 0x1234:0x5678
    mov ah, 0x25
    mov al, 0xFF  ; Use INT FFh as test (unused)
    mov dx, 0x5678
    push ds
    mov bx, 0x1234
    mov ds, bx
    int 0x21
    pop ds

    ; Verify it was set
    mov ah, 0x35
    mov al, 0xFF
    int 0x21
    ; ES:BX should now be 1234:5678

    ; Print message
    mov dx, msg_set_vector
    mov ah, 0x09
    int 0x21

    ; Print segment
    mov ax, es
    call print_hex_word

    mov dl, ':'
    mov ah, 0x02
    int 0x21

    ; Print offset
    mov ax, bx
    call print_hex_word

    mov dx, msg_newline
    mov ah, 0x09
    int 0x21

    ; Exit
    mov ah, 0x4C
    mov al, 0
    int 0x21

; Print AX as 4-digit hex
print_hex_word:
    push ax
    push bx
    push cx
    push dx

    mov cx, 4  ; 4 hex digits
    .loop:
        rol ax, 4  ; Rotate left by 4 bits
        push ax
        and al, 0x0F
        add al, '0'
        cmp al, '9'
        jle .print
        add al, 7  ; 'A'-'9'-1
    .print:
        mov dl, al
        mov ah, 0x02
        int 0x21
        pop ax
        loop .loop

    pop dx
    pop cx
    pop bx
    pop ax
    ret

msg_version: db 'DOS Version: $'
msg_drive: db 'Current Drive: $'
msg_vector: db 'INT 21h vector: $'
msg_set_vector: db 'Set INT FFh to: $'
msg_newline: db 0x0D, 0x0A, '$'
