; INT 10h Function 15h - Return Physical Display Parameters
; VGA-only function. Returns display adapter info:
;   AL = 15h (function supported)
;   BH = active display code (08h = VGA with color analog display)
;   BL = alternate display code (00h = none)

[CPU 8086]
org 0x0100

start:
    ; Ensure we are in mode 3 (80x25 color text)
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    ; Call Return Physical Display Parameters
    mov ah, 0x15
    int 0x10

    ; AL = 15h means the function is supported
    cmp al, 0x15
    jne fail

    ; BH = 08h: VGA with color analog display
    cmp bh, 0x08
    jne fail

    ; BL = 00h: no alternate display
    cmp bl, 0x00
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
