; logical_demo.asm - Demonstrates AND, OR, XOR, NOT, and TEST
; This program shows bitwise logical operations

; Test AND - used for masking bits
MOV AL, 0b11110000 ; AL = 0xF0
MOV BL, 0b10101010 ; BL = 0xAA
AND AL, BL         ; AL = 0xF0 & 0xAA = 0xA0

; Test OR - used for setting bits
MOV CL, 0b00001111 ; CL = 0x0F
MOV DL, 0b11110000 ; DL = 0xF0
OR CL, DL          ; CL = 0x0F | 0xF0 = 0xFF

; Test XOR - used for toggling bits (and zeroing registers)
MOV AH, 0x55
MOV BH, 0x55
XOR AH, BH         ; AH = 0x55 ^ 0x55 = 0x00

; Common idiom: XOR register with itself to zero it
MOV CH, 0xFF
XOR CH, CH         ; CH = 0 (faster than MOV CH, 0)

; Test NOT - bitwise complement
MOV DH, 0xAA
NOT DH             ; DH = ~0xAA = 0x55

; Test TEST - like AND but doesn't store result, only sets flags
MOV AX, 0x00FF
TEST AX, 0x0080    ; Test if bit 7 is set (sets flags but doesn't modify AX)
                   ; Result: ZF=0 (not zero), SF=1 (bit 7 is set)

HLT

; Expected final register state:
;   AX = 0x00FF (set by MOV before TEST, overwrites earlier results)
;   BX = 0x55AA (BH=0x55, BL=0xAA)
;   CX = 0x00FF (CH=0x00 from XOR, CL=0xFF from OR)
;   DX = 0x55F0 (DH=0x55 from NOT, DL=0xF0)
;
; Intermediate results (overwritten):
;   AL was 0xA0 (from AND) before MOV AX, 0x00FF
;   AH was 0x00 (from XOR) before MOV AX, 0x00FF
;   CH was 0x00 (from XOR CH, CH)
;
; Expected flags from final TEST (AX & 0x0080):
;   Zero Flag (ZF) = 0 (result is not zero: 0x00FF & 0x0080 = 0x0080)
;   Sign Flag (SF) = 1 (bit 7 is set in result)
;   Parity Flag (PF) = 0 (0x80 has 1 bit set, odd parity)
;   Carry Flag (CF) = 0 (cleared by TEST)
;   Overflow Flag (OF) = 0 (cleared by TEST)