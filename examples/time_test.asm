; INT 1Ah Time Services Test
; Tests AH=00h (Get System Time) and AH=01h (Set System Time)

org 0x0100

section .text
start:
    ; Display banner
    mov dx, msg_banner
    call print_string

    ; Test 1: Get initial system time
    mov dx, msg_get_time1
    call print_string

    mov ah, 0x00           ; Function 00h - Get system time
    int 0x1A               ; Call time service

    ; CX:DX now contains tick count, AL contains midnight flag
    push ax                ; Save midnight flag
    push cx                ; Save high word
    push dx                ; Save low word

    ; Display tick count (CX:DX)
    mov dx, msg_ticks
    call print_string
    pop dx                 ; Restore low word for display
    pop cx                 ; Restore high word for display
    call print_dword       ; Print CX:DX as 32-bit value
    call print_newline

    ; Display midnight flag
    pop ax                 ; Restore midnight flag in AL
    mov dx, msg_midnight
    call print_string
    and al, 0xFF
    call print_byte
    call print_newline
    call print_newline

    ; Test 2: Set system time to a specific value
    mov dx, msg_set_time
    call print_string

    ; Set time to 0x00012345 (75,589 ticks = about 1 hour, 9 minutes)
    mov cx, 0x0001         ; High word
    mov dx, 0x2345         ; Low word
    mov ah, 0x01           ; Function 01h - Set system time
    int 0x1A               ; Call time service

    mov dx, msg_set_done
    call print_string
    call print_newline

    ; Test 3: Get system time again to verify
    mov dx, msg_get_time2
    call print_string

    mov ah, 0x00           ; Function 00h - Get system time
    int 0x1A               ; Call time service

    ; Save the results
    push ax                ; Save midnight flag
    push cx                ; Save high word
    push dx                ; Save low word

    ; Display new tick count
    mov dx, msg_ticks
    call print_string
    pop dx                 ; Restore low word
    pop cx                 ; Restore high word
    call print_dword       ; Print CX:DX as 32-bit value
    call print_newline

    ; Display midnight flag (should be 0 after set)
    pop ax                 ; Restore midnight flag
    mov dx, msg_midnight
    call print_string
    and al, 0xFF
    call print_byte
    call print_newline
    call print_newline

    ; Test 4: Set time to just before midnight rollover
    mov dx, msg_set_midnight
    call print_string

    ; Set time to 0x001800B0 (1,573,040 ticks = exactly 24 hours)
    ; This should cause midnight flag to be set on next increment
    mov cx, 0x0018         ; High word
    mov dx, 0x00B0         ; Low word
    mov ah, 0x01           ; Function 01h - Set system time
    int 0x1A               ; Call time service

    mov dx, msg_set_done
    call print_string
    call print_newline

    ; Verify the time
    mov dx, msg_get_time3
    call print_string

    mov ah, 0x00           ; Function 00h - Get system time
    int 0x1A               ; Call time service

    ; Save the results
    push cx
    push dx

    mov dx, msg_ticks
    call print_string
    pop dx
    pop cx
    call print_dword
    call print_newline

    ; Exit program
    mov dx, msg_done
    call print_string

    mov ax, 0x4C00         ; Exit with code 0
    int 0x21

; Print null-terminated string at DS:DX
print_string:
    push ax
    push dx
    mov ah, 0x09           ; DOS print string function
    int 0x21
    pop dx
    pop ax
    ret

; Print newline
print_newline:
    push dx
    mov dx, msg_newline
    call print_string
    pop dx
    ret

; Print 32-bit value in CX:DX as hex
print_dword:
    push ax
    push cx
    push dx

    ; Print high word (CX)
    mov dx, cx
    call print_word

    ; Print low word (DX)
    pop dx                 ; Restore original DX
    push dx
    call print_word

    pop dx
    pop cx
    pop ax
    ret

; Print 16-bit value in DX as hex
print_word:
    push ax
    push dx

    ; Print high byte
    mov al, dh
    call print_byte

    ; Print low byte
    mov al, dl
    call print_byte

    pop dx
    pop ax
    ret

; Print byte in AL as hex
print_byte:
    push ax
    push dx

    ; Print high nibble
    mov dl, al
    shr dl, 4
    call print_hex_digit

    ; Print low nibble
    mov dl, al
    and dl, 0x0F
    call print_hex_digit

    pop dx
    pop ax
    ret

; Print hex digit (0-15) in DL
print_hex_digit:
    push ax
    push dx

    cmp dl, 9
    jle .decimal
    ; It's A-F
    add dl, 'A' - 10
    jmp .print
.decimal:
    add dl, '0'
.print:
    mov ah, 0x02           ; DOS write character
    int 0x21

    pop dx
    pop ax
    ret

section .data
msg_banner      db 'INT 1Ah Time Services Test', 13, 10, '$'
msg_get_time1   db 'Test 1: Get initial system time', 13, 10, '$'
msg_get_time2   db 'Test 2: Get system time after set', 13, 10, '$'
msg_get_time3   db 'Test 3: Verify midnight value', 13, 10, '$'
msg_set_time    db 'Setting system time to 0x00012345...', 13, 10, '$'
msg_set_midnight db 'Setting system time to midnight value (0x001800B0)...', 13, 10, '$'
msg_set_done    db 'Time set successfully!', 13, 10, '$'
msg_ticks       db '  Tick count: 0x$'
msg_midnight    db '  Midnight flag: 0x$'
msg_newline     db 13, 10, '$'
msg_done        db 13, 10, 'All tests completed!', 13, 10, '$'
