; logical_simple.asm - Simple test of logical operations

; Test AND
MOV AL, 0xFF
AND AL, 0xAA       ; AL = 0xFF & 0xAA = 0xAA

; Test OR
MOV BL, 0x0F
OR BL, 0xF0        ; BL = 0x0F | 0xF0 = 0xFF

; Test XOR
MOV CL, 0xAA
XOR CL, 0x55       ; CL = 0xAA ^ 0x55 = 0xFF

; Test XOR to zero
MOV DL, 0x42
XOR DL, DL         ; DL = 0x42 ^ 0x42 = 0x00

; Test NOT
MOV AH, 0xF0
NOT AH             ; AH = ~0xF0 = 0x0F

; Test TEST (doesn't modify register)
MOV BH, 0x80
TEST BH, 0x80      ; BH & 0x80 = 0x80 (SF=1 for 8-bit)

HLT

; Expected final register state:
;   AL = 0xAA
;   AH = 0x0F
;   BL = 0xFF
;   BH = 0x80
;   CL = 0xFF
;   DL = 0x00
;
; Expected flags from TEST BH, 0x80:
;   ZF = 0 (result is not zero)
;   SF = 1 (bit 7 is set for 8-bit result)
;   PF = 0 (0x80 has 1 bit, odd parity)