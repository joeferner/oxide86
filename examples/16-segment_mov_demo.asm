; Demonstration of MOV instructions with segment registers
; Tests opcodes 8C (MOV r/m16, segreg) and 8E (MOV segreg, r/m16)

[bits 16]
[org 0x0100]

start:
    ; Initialize some segment registers with known values
    ; We'll use the data segment (DS) register

    ; Test 1: MOV r/m16, segreg (opcode 8C)
    ; Save the current DS value to AX
    mov ax, ds          ; 8C D8 - MOV AX, DS

    ; Test 2: MOV r/m16, segreg to memory
    ; Save ES to memory location
    mov [test_value], es    ; 8C 06 [offset] - MOV [test_value], ES

    ; Test 3: MOV segreg, r/m16 (opcode 8E)
    ; Set up a new value in BX and move it to ES
    mov bx, 0x1000      ; Load test value into BX
    mov es, bx          ; 8E C3 - MOV ES, BX

    ; Test 4: MOV segreg from memory
    ; Load a value from memory into DS
    mov word [test_value2], 0x2000
    mov ds, [test_value2]   ; 8E 1E [offset] - MOV DS, [test_value2]

    ; Test 5: Move DS to a register to verify
    mov cx, ds          ; 8C D9 - MOV CX, DS (should be 0x2000)

    ; Test 6: Copy segment register through general register
    mov ax, ss          ; 8C D0 - MOV AX, SS
    mov es, ax          ; 8E C0 - MOV ES, AX

    ; Test 7: Move ES to memory and back
    mov [test_value3], es   ; Save ES
    mov dx, [test_value3]   ; Load it back to DX

    ; Halt
    hlt

section .data
test_value:     dw 0x0000
test_value2:    dw 0x0000
test_value3:    dw 0x0000
