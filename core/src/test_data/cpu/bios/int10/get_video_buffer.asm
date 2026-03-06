; INT 10h Function FEh - Get Video Buffer
; Used by programs (e.g. screen readers) to find the actual video buffer address.
; On entry: ES = segment of the virtual video buffer the caller wants to check.
; On exit:  ES:DI = address of the actual video buffer.
; If no virtual buffer is active, ES is unchanged and DI=0.
;
; In color text mode (mode 3): video segment is 0xB800.

[CPU 8086]
org 0x0100

start:
    ; Load ES with the standard color text video segment
    mov ax, 0xB800
    mov es, ax

    ; Call Get Video Buffer
    mov ah, 0xFE
    int 0x10

    ; ES should still be 0xB800 (no virtual buffer redirection)
    mov ax, es
    cmp ax, 0xB800
    jne fail

    ; DI should be 0 (start of the buffer)
    cmp di, 0
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
