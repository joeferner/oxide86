; Modem hangup test:
;   1. Initialize COM1 at 1200 baud
;   2. Send "ATDT0\r", drain the full response (echo + result)
;   3. Send "ATH\r"
;   4. Read echo "ATH\r\n" (5 bytes) and result "OK\r\n" (4 bytes)
;   5. Exit 0 on match, 1 on timeout or mismatch
;
; skip_line reads bytes until LF (0x0A), discarding all of them.
; Returns AX=0 on success, AX=1 on timeout.

[CPU 8086]
org 0x0100

start:
    ; Initialize COM1: 1200 baud, no parity, 1 stop bit, 8 data bits
    mov ah, 0x00
    mov al, 0x43
    mov dx, 0x00
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

    ; Write 'D'
    mov ah, 0x01
    mov al, 'D'
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

    ; Write '0'
    mov ah, 0x01
    mov al, '0'
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

    ; Drain echo line (ATDT0\r\n)
    call skip_line
    test ax, ax
    jnz fail

    ; Drain result line (e.g. NO DIALTONE\r\n)
    call skip_line
    test ax, ax
    jnz fail

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

    ; Write 'H'
    mov ah, 0x01
    mov al, 'H'
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

    ; Read echo: 'A','T','H',CR,LF
    mov si, echo_str
    mov cx, echo_len
.read_echo:
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, [si]
    jne fail
    inc si
    loop .read_echo

    ; Read result: "OK\r\n"
    mov si, result_str
    mov cx, result_len
.read_result:
    mov ah, 0x02
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail
    cmp al, [si]
    jne fail
    inc si
    loop .read_result

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

echo_str:   db 'A','T','H',0x0D,0x0A
echo_len    equ $ - echo_str

result_str: db 'O','K',0x0D,0x0A
result_len  equ $ - result_str
