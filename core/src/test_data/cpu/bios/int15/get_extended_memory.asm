; INT 15h AH=88h - Get Extended Memory Size
; System is configured with 2MB RAM total.
; Extended memory = 2048 KB total - 1024 KB (1MB) = 1024 KB
; CF should be clear on success and AX should be 1024.

[CPU 8086]
org 0x0100

start:
    mov ah, 0x88        ; Function 88h: Get extended memory size
    int 0x15

    ; CF must be clear on success
    jc  fail

    ; AX should be 1024 (KB above 1 MB)
    cmp ax, 1024
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
