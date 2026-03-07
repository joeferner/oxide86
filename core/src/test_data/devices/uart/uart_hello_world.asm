; UART hello world test:
;   1. Initialize COM1 (INT 14h AH=00h): 9600 baud, 8-N-1
;   2. Write "hello" byte by byte (INT 14h AH=01h)
;   3. Read back two bytes and verify they equal "ok" (INT 14h AH=02h)
;
; INT 14h AH=01h (write char): AL=char, DX=port -> AH=LSR (bit 7 = error/timeout)
; INT 14h AH=02h (read char):  DX=port          -> AL=char, AH=LSR (bit 7 = error/timeout)

[CPU 8086]
org 0x0100          ; .COM files start at CS:0100h

start:
    ; --- Initialize COM1: 9600 baud (111), no parity (00), 1 stop bit (0), 8-bit (11) ---
    mov ah, 0x00
    mov al, 0xE3    ; 1110_0011
    mov dx, 0x00    ; COM1
    int 0x14

    ; --- Write 'h' ---
    mov ah, 0x01
    mov al, 'h'
    mov dx, 0x00
    int 0x14
    test ah, 0x80   ; bit 7 = transmit error / timeout
    jnz fail

    ; --- Write 'e' ---
    mov ah, 0x01
    mov al, 'e'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    ; --- Write 'l' ---
    mov ah, 0x01
    mov al, 'l'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    ; --- Write 'l' ---
    mov ah, 0x01
    mov al, 'l'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    ; --- Write 'o' ---
    mov ah, 0x01
    mov al, 'o'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    ; --- Read first reply byte, expect 'o' ---
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80   ; bit 7 = receive error / timeout
    jnz fail
    cmp al, 'o'
    jne fail

    ; --- Read second reply byte, expect 'k' ---
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, 'k'
    jne fail

    ; Success: exit with return code 0
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    ; Failure: exit with return code 1
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
