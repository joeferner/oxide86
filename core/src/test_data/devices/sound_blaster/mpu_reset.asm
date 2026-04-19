; mpu_reset.asm — MPU-401 reset and UART mode entry
;
; 1. Send reset command 0xFF to port 0x331
; 2. Poll port 0x331 bit 7 until data available
; 3. Read from 0x330 — must be 0xFE (ACK)
; 4. Send UART mode command 0x3F to 0x331
; 5. Poll port 0x331 bit 7 until data available
; 6. Read from 0x330 — must be 0xFE (ACK)
;
; Exit: 0=pass, 1=reset ACK wrong, 2=UART ACK wrong

[CPU 8086]
org 0x100

start:
    ; Send reset
    mov dx, 0x331
    mov al, 0xFF
    out dx, al

    ; Poll status bit 7 (data available = bit 7 set)
    mov cx, 5000
.poll1:
    in al, dx
    test al, 0x80
    jnz .read1
    loop .poll1
.read1:
    mov dx, 0x330
    in al, dx
    cmp al, 0xFE
    je .uart_cmd
    mov al, 0x01
    jmp .exit

.uart_cmd:
    mov dx, 0x331
    mov al, 0x3F
    out dx, al
    mov cx, 5000
.poll2:
    in al, dx
    test al, 0x80
    jnz .read2
    loop .poll2
.read2:
    mov dx, 0x330
    in al, dx
    cmp al, 0xFE
    je .pass
    mov al, 0x02
    jmp .exit

.pass:
    xor al, al
.exit:
    mov ah, 0x4C
    int 0x21
