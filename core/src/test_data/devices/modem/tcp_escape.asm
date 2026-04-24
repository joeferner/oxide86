; Modem +++ escape test:
;   1. Initialize COM1 at 1200 baud
;   2. Send "ATDT0\r", drain echo line, read "CONNECT\r\n"
;   3. Send "+++" in data mode — modem escapes to command mode, DCD stays high
;   4. Read "OK\r\n" confirming command mode
;   5. Send "AT\r" — modem echoes and responds with OK
;   6. Read "AT\r\n" (echo) and "OK\r\n" (result)
;   7. Exit 0 on match, 1 on timeout or mismatch

[CPU 8086]
org 0x0100

start:
    ; Initialize COM1: 1200 baud, no parity, 1 stop bit, 8 data bits
    mov ah, 0x00
    mov al, 0x43
    mov dx, 0x00
    int 0x14

    ; Send "ATDT0\r"
    mov si, dial_cmd
    mov cx, dial_len
.send_dial:
    mov ah, 0x01
    mov al, [si]
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    inc si
    loop .send_dial

    ; Drain echo line (ATDT0\r\n)
    call skip_line
    test ax, ax
    jnz fail

    ; Read and verify "CONNECT\r\n"
    mov si, connect_str
    mov cx, connect_len
.read_connect:
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, [si]
    jne fail
    inc si
    loop .read_connect

    ; Send "+++" — triggers escape to command mode (not forwarded to TCP)
    mov ah, 0x01
    mov al, '+'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    mov ah, 0x01
    mov al, '+'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    mov ah, 0x01
    mov al, '+'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    ; Read "OK\r\n" — modem confirms command mode
    mov si, ok_str
    mov cx, ok_len
.read_ok1:
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, [si]
    jne fail
    inc si
    loop .read_ok1

    ; Send "AT\r" in command mode
    mov ah, 0x01
    mov al, 'A'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    mov ah, 0x01
    mov al, 'T'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    mov ah, 0x01
    mov al, 0x0D
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    ; Read echo "AT\r\n" and result "OK\r\n" (8 bytes total)
    mov si, at_response
    mov cx, at_response_len
.read_at:
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, [si]
    jne fail
    inc si
    loop .read_at

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

; skip_line: reads and discards bytes until LF (0x0A).
; Returns AX=0 on success, AX=1 on timeout.
skip_line:
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz .timeout
    cmp al, 0x0A
    jne skip_line
    xor ax, ax
    ret
.timeout:
    mov ax, 1
    ret

dial_cmd:    db 'A','T','D','T','0',0x0D
dial_len     equ $ - dial_cmd

connect_str: db 'C','O','N','N','E','C','T',0x0D,0x0A
connect_len  equ $ - connect_str

ok_str:      db 'O','K',0x0D,0x0A
ok_len       equ $ - ok_str

at_response: db 'A','T',0x0D,0x0A,'O','K',0x0D,0x0A
at_response_len equ $ - at_response
