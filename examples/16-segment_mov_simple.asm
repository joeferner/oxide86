; Simple test of MOV instructions with segment registers
; Tests opcodes 8C (MOV r/m16, segreg) and 8E (MOV segreg, r/m16)

[bits 16]
[org 0x0100]

start:
    ; Test 1: Load 0x1234 into ES using a general register
    mov ax, 0x1234      ; AX = 0x1234
    mov es, ax          ; ES = AX (0x1234)

    ; Test 2: Copy ES back to BX
    mov bx, es          ; BX = ES (should be 0x1234)

    ; Test 3: Load 0x5678 into DS
    mov cx, 0x5678      ; CX = 0x5678
    mov ds, cx          ; DS = CX (0x5678)

    ; Test 4: Copy DS to DX
    mov dx, ds          ; DX = DS (should be 0x5678)

    ; Halt - at this point:
    ; AX = 0x1234
    ; BX = 0x1234 (copied from ES)
    ; CX = 0x5678
    ; DX = 0x5678 (copied from DS)
    ; ES = 0x1234
    ; DS = 0x5678
    hlt
