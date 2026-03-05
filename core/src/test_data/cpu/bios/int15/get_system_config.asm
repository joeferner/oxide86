; INT 15h AH=C0h - Get System Configuration Parameters
; Returns ES:BX pointing to the system descriptor table in ROM (F000:E000).
; Table layout (10 bytes):
;   [0-1] = 0x0008 (length: 8 bytes, little-endian)
;   [2]   = 0xFF   (model: PC)
;   [3]   = 0x00   (submodel)
;   [4]   = 0x01   (BIOS revision 1)
;   [5-9] = 0x00   (feature bytes)
; CF must be clear on success.

[CPU 8086]
org 0x0100

start:
    mov ah, 0xC0        ; Function C0h: Get system config
    int 0x15

    ; CF must be clear
    jc  fail

    ; ES must be 0xF000
    mov ax, es
    cmp ax, 0xF000
    jne fail

    ; BX must be 0xE000
    cmp bx, 0xE000
    jne fail

    ; Check table length field (word at ES:BX) = 0x0008
    mov ax, [es:bx]
    cmp ax, 0x0008
    jne fail

    ; Check model byte (offset 2) = 0xFF (PC)
    mov al, [es:bx+2]
    cmp al, 0xFF
    jne fail

    ; Check submodel byte (offset 3) = 0x00
    mov al, [es:bx+3]
    cmp al, 0x00
    jne fail

    ; Check BIOS revision (offset 4) = 0x01
    mov al, [es:bx+4]
    cmp al, 0x01
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
