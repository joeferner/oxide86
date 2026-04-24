; Modem AT basic test:
;   1. Initialize COM1 at 1200 baud (INT 14h AH=00h)
;   2. Write "AT\r" byte by byte (INT 14h AH=01h)
;   3. Read response and verify it equals "AT\r\nOK\r\n" (echo + result code)
;   4. Exit 0 on match, 1 on timeout or mismatch
;
; 1200 baud = bits 7-5 = 010, no parity = 00, 1 stop = 0, 8-bit = 11 => AL = 0100_0011 = 0x43

[CPU 8086]
org 0x0100

start:
    ; Initialize COM1: 1200 baud, no parity, 1 stop bit, 8 data bits
    mov ah, 0x00
    mov al, 0x43
    mov dx, 0x00    ; COM1
    int 0x14

    ; Write 'A'
    mov ah, 0x01
    mov al, 'A'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    ; Write 'T'
    mov ah, 0x01
    mov al, 'T'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    ; Write CR
    mov ah, 0x01
    mov al, 0x0D
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    ; Read 'A' (echo)
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 'A'
    jne fail

    ; Read 'T' (echo)
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 'T'
    jne fail

    ; Read CR (echo)
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 0x0D
    jne fail

    ; Read LF (echo — CR expands to CR+LF)
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 0x0A
    jne fail

    ; Read 'O'
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 'O'
    jne fail

    ; Read 'K'
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 'K'
    jne fail

    ; Read CR
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 0x0D
    jne fail

    ; Read LF
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 0x0A
    jne fail

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
