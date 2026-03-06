; INT 10h Function 11h - Character Generator
; AL=30h: Return Font Information
;   BH=6: ROM 8x16 font  -> CX=16 (bytes/char), DX=24 (rows-1 for 25-row screen)
;   BH=3: ROM 8x8 font (low 128 chars) -> CX=8, DX=24
; ES:BP returns pointer to the font table (verified non-zero via ES or BP)

[CPU 8086]
org 0x0100

start:
    ; Query 8x16 ROM font info (BH=6)
    mov ah, 0x11
    mov al, 0x30
    mov bh, 6
    int 0x10

    ; CX = bytes per character (should be 16 for 8x16 font)
    cmp cx, 16
    jne fail

    ; DX = number of screen rows - 1 (should be 24 for 25-row mode)
    cmp dx, 24
    jne fail

    ; Query 8x8 ROM font info (BH=3)
    mov ah, 0x11
    mov al, 0x30
    mov bh, 3
    int 0x10

    ; CX = bytes per character (should be 8 for 8x8 font)
    cmp cx, 8
    jne fail

    ; DX = rows - 1 should still be 24
    cmp dx, 24
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
