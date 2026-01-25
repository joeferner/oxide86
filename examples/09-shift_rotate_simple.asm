; shift_rotate_simple.asm - Simple test of shift/rotate operations
; Tests: SHL, SHR, SAR, ROL, ROR (register-to-register only)

; === Test SHL (Shift Left) ===
; SHL shifts bits left, filling with zeros
MOV AL, 0x0F        ; AL = 15
SHL AL, 1           ; AL = 0x1E = 30, CF=0

MOV BL, 0x81        ; BL = 129
SHL BL, 1           ; BL = 0x02 = 2, CF=1 (bit shifted out)

; Test 16-bit SHL
MOV AX, 0x0F0F      ; AX = 3855
SHL AX, 1           ; AX = 0x1E1E = 7710, CF=0

; === Test SHR (Shift Right Logical) ===
; SHR shifts bits right, filling with zeros
MOV CL, 0xF0        ; CL = 240
SHR CL, 1           ; CL = 0x78 = 120, CF=0

MOV DL, 0x0F        ; DL = 15
SHR DL, 1           ; DL = 0x07 = 7, CF=1 (bit shifted out)

; === Test SAR (Shift Right Arithmetic) ===
; SAR shifts bits right, preserving sign bit
MOV CH, 0xF0        ; CH = -16 (signed)
SAR CH, 1           ; CH = 0xF8 = -8 (sign preserved), CF=0

MOV DH, 0x0E        ; DH = 14 (positive)
SAR DH, 1           ; DH = 0x07 = 7, CF=0

; === Test ROL (Rotate Left) ===
; ROL rotates bits left, bit 7 goes to bit 0 and CF
MOV AL, 0x81        ; AL = 129
ROL AL, 1           ; AL = 0x03 = 3, CF=1 (bit rotated from MSB)

; === Test ROR (Rotate Right) ===
; ROR rotates bits right, bit 0 goes to bit 7 and CF
MOV BL, 0x81        ; BL = 129
ROR BL, 1           ; BL = 0xC0 = 192, CF=1 (bit rotated from LSB)

; === Test multiple shifts using CL ===
MOV CL, 3           ; Shift count in CL
MOV AL, 0x01        ; AL = 1
SHL AL, CL          ; AL = 0x08 = 8 (shifted left 3 times)

MOV BL, 0x80        ; BL = 128
SHR BL, CL          ; BL = 0x10 = 16 (shifted right 3 times)

; === Test 16-bit rotates ===
MOV BX, 0x8001      ; BX = 32769
ROL BX, 1           ; BX = 0x0003, CF=1

MOV DX, 0x8001      ; DX = 32769
ROR DX, 1           ; DX = 0xC000, CF=1

; === Test RCL/RCR (Rotate through Carry) ===
; First, set up carry flag
MOV AL, 0xFF
SHL AL, 1           ; Sets CF=1, AL=0xFE

; Now test RCL (rotate left through carry)
MOV AL, 0x80        ; AL = 0b10000000
RCL AL, 1           ; AL = 0b00000001 (MSB->CF, old CF->LSB), CF=1

; Test RCR (rotate right through carry)
MOV BL, 0x01        ; BL = 0b00000001
RCR BL, 1           ; BL = 0b10000000 (LSB->CF, old CF->MSB), CF=1

HLT

; Expected final register state (key values):
;   AL should have been modified by RCL test
;   BL should have been modified by RCR test
;   BX = 0x0003 (0x8001 rotated left)
;   DX = 0xC000 (0x8001 rotated right)
;
; This test exercises:
; - SHL (shift left logical/arithmetic)
; - SHR (shift right logical)
; - SAR (shift right arithmetic, preserves sign)
; - ROL (rotate left)
; - ROR (rotate right)
; - RCL (rotate through carry left)
; - RCR (rotate through carry right)
; - Both 8-bit and 16-bit operations
; - Shifts by 1 and by CL register
