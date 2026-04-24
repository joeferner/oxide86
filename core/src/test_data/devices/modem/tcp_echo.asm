; Modem TCP echo test:
;   1. Initialize COM1 at 1200 baud
;   2. Send "ATDT0\r"
;   3. Drain echo line (ATDT0\r\n = 7 bytes)
;   4. Read and verify "CONNECT\r\n"
;   5. Send "Hello\r" in data mode (forwarded to TCP echo server)
;   6. Read back "Hello\r" echoed by server
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

    ; Send "Hello\r" to TCP echo server (data mode — forwarded directly)
    mov si, hello_str
    mov cx, hello_len
.send_hello:
    mov ah, 0x01
    mov al, [si]
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    inc si
    loop .send_hello

    ; Read back "Hello\r" from TCP echo server
    mov si, hello_str
    mov cx, hello_len
.read_hello:
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, [si]
    jne fail
    inc si
    loop .read_hello

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

hello_str:   db 'H','e','l','l','o',0x0D
hello_len    equ $ - hello_str
