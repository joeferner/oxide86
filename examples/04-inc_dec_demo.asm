; inc_dec_demo.asm - Demonstrates INC and DEC instructions
; This program tests increment and decrement operations

; Test INC with 8-bit register
MOV AL, 5          ; AL = 5
INC AL             ; AL = 6

; Test DEC with 8-bit register
MOV BL, 10         ; BL = 10
DEC BL             ; BL = 9

; Test INC with 16-bit register (using register encoding)
MOV CX, 100        ; CX = 100
INC CX             ; CX = 101

; Test DEC with 16-bit register (using register encoding)
MOV DX, 200        ; DX = 200
DEC DX             ; DX = 199

; Test loop using DEC (cleaner than SUB)
MOV CL, 5          ; CL = 5
count_down:
    DEC CL         ; CL = CL - 1
    CMP CL, 0      ; Compare with 0
    JNZ count_down ; Loop if not zero

HLT

; Expected final register state:
;   AL = 0x06 (6 decimal)
;   BL = 0x09 (9 decimal)
;   CL = 0x00 (0 decimal)
;   CX = 0x0000 (0 decimal - CL overwrites low byte from loop)
;   DX = 0x00C7 (199 decimal)
;
; Expected flags:
;   Zero Flag (ZF) = 1 (CL is zero after loop)
