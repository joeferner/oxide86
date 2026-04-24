; Modem TCP disconnect test:
;   1. Initialize COM1 at 1200 baud
;   2. Send "ATDT0\r", drain echo line
;   3. Read and verify "CONNECT\r\n"
;   4. Send "Hi\r" (3 bytes) — echo server echoes and then closes
;   5. Read back "Hi\r" (echo from server)
;   6. Read "NO CARRIER\r\n" queued by modem when TCP closes
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

    ; Send "Hi\r" — server echoes and then closes the connection
    mov si, hi_str
    mov cx, hi_len
.send_hi:
    mov ah, 0x01
    mov al, [si]
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    inc si
    loop .send_hi

    ; Read back "Hi\r" echoed by server
    mov si, hi_str
    mov cx, hi_len
.read_hi:
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, [si]
    jne fail
    inc si
    loop .read_hi

    ; Read "NO CARRIER\r\n" queued after TCP close
    mov si, nocarrier_str
    mov cx, nocarrier_len
.read_nocarrier:
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, [si]
    jne fail
    inc si
    loop .read_nocarrier

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

dial_cmd:      db 'A','T','D','T','0',0x0D
dial_len       equ $ - dial_cmd

connect_str:   db 'C','O','N','N','E','C','T',0x0D,0x0A
connect_len    equ $ - connect_str

hi_str:        db 'H','i',0x0D
hi_len         equ $ - hi_str

nocarrier_str: db 'N','O',' ','C','A','R','R','I','E','R',0x0D,0x0A
nocarrier_len  equ $ - nocarrier_str
