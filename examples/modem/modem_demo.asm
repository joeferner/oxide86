; Modem demo program:
;   1. Init COM1 at 1200 baud
;   2. ATZ\r  → drain echo + OK
;   3. ATDT0\r → drain echo + CONNECT
;   4. Send "Hello, modem world!\r\n" in data mode
;   5. Read back the echo, print each byte to the screen
;   6. Send "+++" → drain OK (escape to command mode)
;   7. ATH\r  → drain echo + NO CARRIER
;   8. Print "Done." and exit 0

[CPU 8086]
org 0x0100

start:
    ; Init COM1: 1200 baud, no parity, 1 stop bit, 8 data bits
    mov ah, 0x00
    mov al, 0x43
    mov dx, 0x00
    int 0x14

    ; ATZ\r
    mov si, atz_cmd
    mov cx, atz_len
    call send_bytes
    test ax, ax
    jnz fail

    ; drain echo (ATZ\r\n)
    call skip_line
    test ax, ax
    jnz fail

    ; drain OK\r\n
    call skip_line
    test ax, ax
    jnz fail

    ; ATDT0\r
    mov si, dial_cmd
    mov cx, dial_len
    call send_bytes
    test ax, ax
    jnz fail

    ; drain echo (ATDT0\r\n)
    call skip_line
    test ax, ax
    jnz fail

    ; drain CONNECT\r\n
    call skip_line
    test ax, ax
    jnz fail

    ; Send "Hello, modem world!\r\n" into data mode
    mov si, hello_str
    mov cx, hello_len
    call send_bytes
    test ax, ax
    jnz fail

    ; Read back the echo, print each byte to the screen
    mov cx, hello_len
.read_echo:
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    push cx
    mov dl, al
    mov ah, 0x02
    int 0x21
    pop cx
    loop .read_echo

    ; Send "+++" to escape to command mode
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

    ; drain OK\r\n (guard-time confirm)
    call skip_line
    test ax, ax
    jnz fail

    ; ATH\r to hang up
    mov si, ath_cmd
    mov cx, ath_len
    call send_bytes
    test ax, ax
    jnz fail

    ; drain echo (ATH\r\n)
    call skip_line
    test ax, ax
    jnz fail

    ; drain NO CARRIER\r\n
    call skip_line
    test ax, ax
    jnz fail

    ; Print "Done."
    mov ah, 0x09
    mov dx, done_msg
    int 0x21

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x09
    mov dx, fail_msg
    int 0x21

    mov ah, 0x4C
    mov al, 0x01
    int 0x21

; send_bytes: sends CX bytes from DS:SI via INT 14h AH=01.
; Returns AX=0 on success, AX=1 on timeout/error.
send_bytes:
.loop:
    mov ah, 0x01
    mov al, [si]
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz .timeout
    inc si
    loop .loop
    xor ax, ax
    ret
.timeout:
    mov ax, 1
    ret

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

atz_cmd:   db 'A','T','Z',0x0D
atz_len    equ $ - atz_cmd

dial_cmd:  db 'A','T','D','T','0',0x0D
dial_len   equ $ - dial_cmd

hello_str: db 'H','e','l','l','o',',',' ','m','o','d','e','m',' ','w','o','r','l','d','!',0x0D,0x0A
hello_len  equ $ - hello_str

ath_cmd:   db 'A','T','H',0x0D
ath_len    equ $ - ath_cmd

done_msg:  db 'Done.',0x0D,0x0A,'$'
fail_msg:  db 'FAIL',0x0D,0x0A,'$'
