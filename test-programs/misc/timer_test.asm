; timer_test.asm - Test TIMER functionality using DOS INT 21h AH=2Ch
; Expected: Should measure approximately 1.0 second delay

[CPU 8086]
org 0x100

start:
    ; Get start time
    mov ah, 0x2C          ; Get system time
    int 0x21
    ; CX = hours:minutes, DX = seconds:hundredths
    push cx
    push dx               ; Save start time

    ; Display "Start: HH:MM:SS.HH"
    mov dx, msg_start
    mov ah, 0x09
    int 0x21

    pop dx
    pop cx
    push cx
    push dx               ; Keep start time on stack
    call print_time

    ; Wait 1 second using INT 15h AH=86h
    mov ah, 0x86          ; Wait
    mov cx, 0x000F        ; High word of microseconds (15 * 65536 = 983040 µs)
    mov dx, 0x4240        ; Low word of microseconds (16960 µs) = 1000000 µs total
    int 0x15

    ; Get end time
    mov ah, 0x2C          ; Get system time
    int 0x21
    ; CX = hours:minutes, DX = seconds:hundredths

    ; Display "End:   HH:MM:SS.HH"
    push cx
    push dx               ; Save end time

    mov dx, msg_end
    mov ah, 0x09
    int 0x21

    pop dx
    pop cx
    call print_time

    ; Calculate elapsed time (end - start)
    ; For simplicity, just show raw values
    pop ax                ; start DX (seconds:hundredths)
    pop bx                ; start CX (hours:minutes)
    ; We have end in CX:DX, start in BX:AX

    mov dx, msg_done
    mov ah, 0x09
    int 0x21

    ; Exit
    mov ah, 0x4C
    int 0x21

; Print time from CX:DX (hours:minutes in CH:CL, seconds:hundredths in DH:DL)
print_time:
    ; Print hours
    mov al, ch
    call print_byte_decimal

    mov dl, ':'
    mov ah, 0x02
    int 0x21

    ; Print minutes
    mov al, cl
    call print_byte_decimal

    mov dl, ':'
    mov ah, 0x02
    int 0x21

    ; Print seconds
    mov al, dh
    call print_byte_decimal

    mov dl, '.'
    mov ah, 0x02
    int 0x21

    ; Print hundredths
    mov al, dl
    call print_byte_decimal

    ; Print newline
    mov dx, crlf
    mov ah, 0x09
    int 0x21

    ret

; Print AL as 2-digit decimal
print_byte_decimal:
    push ax
    push dx

    xor ah, ah            ; Clear AH
    mov dl, 10
    div dl                ; AL = quotient (tens), AH = remainder (ones)

    ; Print tens
    add al, '0'
    mov dl, al
    push ax
    mov ah, 0x02
    int 0x21
    pop ax

    ; Print ones
    mov dl, ah
    add dl, '0'
    mov ah, 0x02
    int 0x21

    pop dx
    pop ax
    ret

msg_start: db 'Start: $'
msg_end:   db 'End:   $'
msg_done:  db 'Done!', 13, 10, '$'
crlf:      db 13, 10, '$'
