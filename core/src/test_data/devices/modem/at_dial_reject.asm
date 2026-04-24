; Modem dial-reject test:
;   1. Initialize COM1 at 1200 baud
;   2. Send "ATDT555\r"
;   3. Read echo "ATDT555\r\n" (9 bytes)
;   4. Read result "NO DIALTONE\r\n" (13 bytes)
;   5. Exit 0 on match, 1 on timeout or mismatch

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

    ; Write '5'
    mov ah, 0x01
    mov al, '5'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    ; Write '5'
    mov ah, 0x01
    mov al, '5'
    mov dx, 0x00
    int 0x14
    test ah, 0x80
    jnz fail

    ; Write '5'
    mov ah, 0x01
    mov al, '5'
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

    ; Read echo: 'A','T','D','T','5','5','5',CR,LF
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

    ; Read result: "NO DIALTONE\r\n"
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

echo_str:   db 'A','T','D','T','5','5','5',0x0D,0x0A
echo_len    equ $ - echo_str

result_str: db 'N','O',' ','D','I','A','L','T','O','N','E',0x0D,0x0A
result_len  equ $ - result_str
