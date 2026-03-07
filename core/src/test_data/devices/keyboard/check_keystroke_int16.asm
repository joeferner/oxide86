; Keyboard test: poll for a keystroke, verify ASCII is 'o' and scan code is 0x18, then exit

[CPU 8086]
org 0x0100          ; .COM files start at CS:0100h

start:

poll:
    mov ah, 0x01    ; INT 16h function 01h: check for keystroke (non-destructive peek)
    int 0x16        ; ZF=1: no key available, ZF=0: key ready (AH=scan, AL=ASCII)
    jz  poll        ; loop until a key is available

    ; A key is available - consume it from the buffer
    mov ah, 0x00    ; INT 16h function 00h: get keystroke (removes key from buffer)
    int 0x16        ; AH = BIOS scan code, AL = ASCII character

    ; Verify the key is 'o' (ASCII 0x6F)
    cmp al, 'o'
    jne fail

    ; Verify the scan code is 0x18 (BIOS scan code for 'o')
    cmp ah, 0x18
    jne fail

    ; Success: exit with return code 0
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    ; Wrong key: exit with return code 1
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
