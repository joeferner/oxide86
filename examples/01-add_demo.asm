; add_demo.asm - Demonstrates MOV and ADD instructions
; This program shows basic data movement and arithmetic operations

; Load immediate values into registers
MOV AL, 5          ; Load 5 into AL (low byte of AX)
MOV BL, 3          ; Load 3 into BL (low byte of BX)

; Move data between registers
MOV CL, AL         ; Copy AL to CL (CL = 5)

; Add register to register
ADD AL, BL         ; Add BL to AL (AL = 5 + 3 = 8)

; Load value into high byte
MOV AH, 2          ; Load 2 into AH (high byte of AX)
                   ; Now AX = 0x0208

; Add immediate to 16-bit register
ADD AX, 0x10       ; Add 16 to AX (AX = 0x0208 + 0x0010 = 0x0218)

; Stop execution
HLT

; Expected final register state:
;   AL = 0x18 (24 decimal)
;   AH = 0x02 (2 decimal)
;   AX = 0x0218 (536 decimal)
;   BL = 0x03 (3 decimal)
;   CL = 0x05 (5 decimal)
;
; Expected flags:
;   Zero Flag (ZF) = 0 (result is not zero)
;   Sign Flag (SF) = 0 (result is positive)
;   Carry Flag (CF) = 0 (no carry out)
;   Overflow Flag (OF) = 0 (no signed overflow)