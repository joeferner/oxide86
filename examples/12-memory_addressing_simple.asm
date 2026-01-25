; Simplified test program for memory addressing modes
; Assumes DS is already set to 0x0000

[BITS 16]
[ORG 0x0100]

start:
    ; Initialize BX to point to a memory location
    mov bx, 0x0200

    ; Test 1: MOV to memory using [BX] - register indirect
    mov al, 0x42
    mov [bx], al               ; Store AL at DS:BX (0x0200)

    ; Test 2: MOV from memory using [BX]
    mov al, 0x00
    mov al, [bx]               ; Load from DS:BX into AL (should be 0x42)

    ; Test 3: MOV with displacement [BX+offset]
    mov byte [bx+1], 0x43      ; Store 0x43 at DS:BX+1 (0x0201)
    mov ah, [bx+1]             ; Load from DS:BX+1 into AH (should be 0x43)
    ; AX should now be 0x4342

    ; Test 4: ADD with memory operand
    mov byte [bx+2], 0x10      ; Store 0x10 at DS:BX+2
    mov al, 0x05
    add al, [bx+2]             ; AL = 0x05 + 0x10 = 0x15

    ; Test 5: SUB with memory operand
    mov byte [bx+3], 0x08
    mov cl, 0x20
    sub cl, [bx+3]             ; CL = 0x20 - 0x08 = 0x18

    ; Test 6: AND with memory operand
    mov byte [bx+4], 0x0F
    mov dl, 0xFF
    and dl, [bx+4]             ; DL = 0xFF & 0x0F = 0x0F

    ; Test 7: OR with memory operand
    mov byte [bx+5], 0xF0
    mov dh, 0x0A
    or dh, [bx+5]              ; DH = 0x0A | 0xF0 = 0xFA

    ; Test 8: XOR with memory operand
    mov byte [bx+6], 0x55
    mov bl, 0xAA
    xor bl, [bx+6]             ; BL = 0xAA ^ 0x55 = 0xFF

    ; Test 9: CMP with memory operand
    mov byte [bx+7], 0x20
    mov ch, 0x20
    cmp ch, [bx+7]             ; Should set ZF (zero flag) since they're equal

    ; Test 10: INC memory operand
    mov byte [bx+8], 0x7F
    inc byte [bx+8]            ; Should become 0x80

    ; Test 11: DEC memory operand
    mov byte [bx+9], 0x01
    dec byte [bx+9]            ; Should become 0x00

    ; Test 12: Direct addressing [offset]
    mov byte [0x0300], 0x99     ; Store at DS:0x0300
    mov al, [0x0300]            ; Load from DS:0x0300 (should be 0x99)

    ; Test 13: Using SI
    mov si, 0x0210
    mov byte [si], 0xAA        ; Store at DS:SI
    mov al, [si]               ; Load from DS:SI (should be 0xAA)

    ; Test 14: Using DI
    mov di, 0x0220
    mov byte [di], 0xBB        ; Store at DS:DI
    mov ah, [di]               ; Load from DS:DI (should be 0xBB)
    ; AX should now be 0xBBAA

    ; Test 15: Based indexed addressing [BX+SI]
    mov bx, 0x0100
    mov si, 0x0010
    mov byte [bx+si], 0xCC     ; Store at DS:BX+SI (0x0110)
    mov al, [bx+si]            ; Load from DS:BX+SI (should be 0xCC)

    hlt
