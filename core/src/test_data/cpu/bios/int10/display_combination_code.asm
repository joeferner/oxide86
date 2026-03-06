; INT 10h Function 1Ah - Display Combination Code (DCC)
; AL=00h: Read DCC
;   Returns AL=1Ah (function supported), BL=active display code,
;   BH=alternate display code
; AL=01h: Write DCC (BL=active, BH=alternate)
;
; VGA with color display: BL=08h

[CPU 8086]
org 0x0100

start:
    ; Read DCC
    mov ah, 0x1A
    mov al, 0x00
    int 0x10

    ; AL=1Ah means the function is supported
    cmp al, 0x1A
    jne fail

    ; BL should be a non-zero display code (VGA color = 8)
    cmp bl, 0
    je  fail

    ; Save the DCC values
    mov ch, bl      ; active display code
    mov cl, bh      ; alternate display code

    ; Write the same values back (AL=01h)
    mov ah, 0x1A
    mov al, 0x01
    mov bl, ch
    mov bh, cl
    int 0x10

    ; Read again - should return same codes
    mov ah, 0x1A
    mov al, 0x00
    int 0x10

    cmp al, 0x1A
    jne fail
    cmp bl, ch
    jne fail
    cmp bh, cl
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
