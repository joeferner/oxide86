; Test program for memory addressing modes
; This tests various addressing modes: direct, register indirect, based indexed, etc.

[BITS 16]
[ORG 0x0000]

start:
    ; Set up segment registers
    mov ax, 0x1000
    mov ds, ax
    mov ss, ax
    mov sp, 0xFFFE

    ; Initialize some data at memory location 0x10000 + 0x0100 = 0x10100
    mov bx, 0x0100

    ; Test 1: MOV to memory using [BX] - register indirect
    mov byte [bx], 0x42        ; Store 0x42 at DS:BX (0x10100)
    mov byte [bx+1], 0x43      ; Store 0x43 at DS:BX+1 (0x10101)

    ; Test 2: MOV from memory using [BX]
    mov al, [bx]               ; Load from DS:BX into AL (should be 0x42)
    mov ah, [bx+1]             ; Load from DS:BX+1 into AH (should be 0x43)
    ; AX should now be 0x4342

    ; Test 3: ADD with memory operand
    mov byte [bx+2], 0x10      ; Store 0x10 at DS:BX+2
    mov al, 0x05
    add al, [bx+2]             ; AL = 0x05 + 0x10 = 0x15

    ; Test 4: SUB with memory operand
    mov byte [bx+3], 0x08
    mov cl, 0x20
    sub cl, [bx+3]             ; CL = 0x20 - 0x08 = 0x18

    ; Test 5: AND with memory operand
    mov byte [bx+4], 0x0F
    mov dl, 0xFF
    and dl, [bx+4]             ; DL = 0xFF & 0x0F = 0x0F

    ; Test 6: OR with memory operand
    mov byte [bx+5], 0xF0
    mov dh, 0x0A
    or dh, [bx+5]              ; DH = 0x0A | 0xF0 = 0xFA

    ; Test 7: XOR with memory operand
    mov byte [bx+6], 0x55
    mov bl, 0xAA
    xor bl, [bx+6]             ; BL = 0xAA ^ 0x55 = 0xFF

    ; Test 8: CMP with memory operand
    mov bx, 0x0100
    mov byte [bx+7], 0x20
    mov ch, 0x20
    cmp ch, [bx+7]             ; Should set ZF (zero flag) since they're equal

    ; Test 9: INC memory operand
    mov byte [bx+8], 0x7F
    inc byte [bx+8]            ; Should become 0x80

    ; Test 10: DEC memory operand
    mov byte [bx+9], 0x01
    dec byte [bx+9]            ; Should become 0x00

    ; Test 11: Direct addressing [offset]
    mov byte [0x200], 0x99     ; Store at DS:0x200
    mov al, [0x200]            ; Load from DS:0x200 (should be 0x99)

    ; Test 12: Using SI and DI
    mov si, 0x0100
    mov di, 0x0150
    mov byte [si], 0xAA        ; Store at DS:SI
    mov byte [di], 0xBB        ; Store at DS:DI
    mov al, [si]               ; Load from DS:SI (should be 0xAA)
    mov ah, [di]               ; Load from DS:DI (should be 0xBB)

    ; Test 13: Based indexed addressing [BX+SI]
    mov bx, 0x0100
    mov si, 0x0010
    mov byte [bx+si], 0xCC     ; Store at DS:BX+SI (0x0110)
    mov al, [bx+si]            ; Load from DS:BX+SI (should be 0xCC)

    hlt
