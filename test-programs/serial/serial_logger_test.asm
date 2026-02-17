; Serial Logger Test Program
; Writes debug output to COM1 to verify serial logger functionality
; Build: nasm -f bin serial_logger_test.asm -o serial_logger_test.com
; Run: cargo run -p emu86-native-cli -- test-programs/serial/serial_logger_test.com --com1-device logger

[CPU 8086]
org 0x100

start:
    ; Initialize COM1: 9600 baud, no parity, 1 stop bit, 8 data bits
    mov dx, 0          ; DX = 0 (COM1)
    mov ah, 0x00       ; Function: Initialize port
    mov al, 0xE3       ; Parameters: 9600,N,1,8 (111_00_0_11b)
    int 0x14           ; Call BIOS serial services

    ; Print startup message
    mov si, msg_start
    call print_string

    ; Test 1: Simple message
    mov si, msg_test1
    call print_string

    ; Test 2: Multiple lines
    mov si, msg_test2a
    call print_string
    mov si, msg_test2b
    call print_string
    mov si, msg_test2c
    call print_string

    ; Test 3: Number output
    mov si, msg_test3
    call print_string
    mov ax, 42
    call print_number
    call print_newline

    mov ax, 1234
    call print_number
    call print_newline

    mov ax, 65535
    call print_number
    call print_newline

    ; Test 4: Mixed content
    mov si, msg_test4
    call print_string

    ; Print completion message
    mov si, msg_done
    call print_string

    ; Exit
    mov ah, 0x4C
    int 0x21

; Print null-terminated string to COM1
; Input: SI = pointer to string
print_string:
    push ax
    push dx
    push si

    mov dx, 0          ; DX = 0 (COM1)
.loop:
    lodsb              ; AL = [SI++]
    test al, al        ; Check for null terminator
    jz .done

    mov ah, 0x01       ; Function: Write character
    int 0x14           ; Call BIOS serial services
    jmp .loop

.done:
    pop si
    pop dx
    pop ax
    ret

; Print newline (CR+LF) to COM1
print_newline:
    push ax
    push dx

    mov dx, 0          ; DX = 0 (COM1)
    mov ah, 0x01       ; Function: Write character
    mov al, 0x0D       ; CR
    int 0x14
    mov ah, 0x01       ; Function: Write character (must reset after INT modifies AH)
    mov al, 0x0A       ; LF
    int 0x14

    pop dx
    pop ax
    ret

; Print 16-bit number in decimal to COM1
; Input: AX = number to print
print_number:
    push ax
    push bx
    push cx
    push dx

    mov cx, 0          ; Digit counter
    mov bx, 10         ; Divisor

    ; Handle zero specially
    test ax, ax
    jnz .convert

    push ax
    mov al, '0'
    mov ah, 0x01
    mov dx, 0
    int 0x14
    pop ax
    jmp .done

.convert:
    ; Convert to decimal digits (stored on stack in reverse)
    test ax, ax
    jz .print

    xor dx, dx         ; DX:AX = number
    div bx             ; AX = quotient, DX = remainder
    push dx            ; Save digit
    inc cx             ; Count digit
    jmp .convert

.print:
    ; Pop and print digits
    test cx, cx
    jz .done

    pop ax             ; Get digit
    add al, '0'        ; Convert to ASCII
    push cx
    mov ah, 0x01       ; Function: Write character
    mov dx, 0          ; COM1
    int 0x14
    pop cx
    dec cx
    jmp .print

.done:
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Messages
msg_start:  db '[TEST] Serial Logger Test Program', 0x0D, 0x0A, 0
msg_test1:  db '[TEST] Test 1: Simple message', 0x0D, 0x0A, 0
msg_test2a: db '[TEST] Test 2: Multiple lines', 0x0D, 0x0A, 0
msg_test2b: db '[TEST]   - Line 2', 0x0D, 0x0A, 0
msg_test2c: db '[TEST]   - Line 3', 0x0D, 0x0A, 0
msg_test3:  db '[TEST] Test 3: Number output: ', 0
msg_test4:  db '[TEST] Test 4: Mixed content - CPU initialized, memory OK, ready to run', 0x0D, 0x0A, 0
msg_done:   db '[TEST] All tests completed successfully!', 0x0D, 0x0A, 0
