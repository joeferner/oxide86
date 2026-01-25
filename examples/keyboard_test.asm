; Keyboard Services Test (INT 16h)
; Tests keyboard input using BIOS INT 16h
;
; Build and run:
;   ./examples/run.sh keyboard_test.asm

org 0x0100                ; COM file format

section .text
start:
    ; Display prompt
    mov dx, prompt
    mov ah, 0x09
    int 0x21

read_loop:
    ; Check for keystroke (AH=01h)
    mov ah, 0x01
    int 0x16
    jz read_loop          ; Jump if ZF=1 (no key available)

    ; Read the character (AH=00h)
    mov ah, 0x00
    int 0x16              ; Returns scan code in AH, ASCII in AL

    ; Check for ESC key (ASCII 0x1B)
    cmp al, 0x1B
    je exit_program

    ; Display the character
    mov dl, al
    mov ah, 0x02
    int 0x21

    ; Display scan code in hex
    mov dx, scan_msg
    mov ah, 0x09
    int 0x21

    ; Convert scan code (in AH) to hex and display
    push ax               ; Save AX
    shr ax, 8             ; Move AH to AL
    call print_hex_byte

    ; Display newline
    mov dx, newline
    mov ah, 0x09
    int 0x21

    pop ax                ; Restore AX
    jmp read_loop

exit_program:
    ; Display exit message
    mov dx, exit_msg
    mov ah, 0x09
    int 0x21

    ; Exit program
    mov ah, 0x4C
    xor al, al
    int 0x21

; Print a byte in AL as hex
print_hex_byte:
    push ax
    ; Print high nibble
    shr al, 4
    call print_hex_nibble
    ; Print low nibble
    pop ax
    and al, 0x0F
    call print_hex_nibble
    ret

; Print a nibble (0-15) in AL as hex digit
print_hex_nibble:
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
    ret

section .data
prompt:     db 'Keyboard Test - Press keys (ESC to exit)', 13, 10, '$'
scan_msg:   db ' [Scan: 0x$'
newline:    db ']', 13, 10, '$'
exit_msg:   db 13, 10, 'Exiting...', 13, 10, '$'
