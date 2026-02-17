; Test program to demonstrate memory value logging
; Assemble with: nasm -f bin test_mem_log.asm -o test_mem_log.com

[CPU 8086]
org 0x100

start:
    ; Store some test values in memory
    mov word [test_value1], 0x1234
    mov byte [test_value2], 0xAB

    ; Now do operations that will show memory values in logs
    mov ax, [test_value1]     ; Should log [test_value1]=1234
    or ax, [0x24bc]           ; Should log AX and [0x24bc]=value
    add bx, [test_value1]     ; Should log BX and [test_value1]=1234

    ; Test with displacement
    mov si, test_values
    mov ax, [si]              ; Should log [si]=value
    mov bx, [si+2]            ; Should log [si+2]=value

    ; Exit
    mov ah, 0x4C
    int 0x21

test_value1: dw 0
test_value2: db 0
test_values: dw 0xBEEF, 0xCAFE, 0xDEAD
