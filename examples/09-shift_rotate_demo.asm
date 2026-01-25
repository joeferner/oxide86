; shift_rotate_demo.asm - Comprehensive test of shift/rotate operations
; Tests: SHL, SHR, SAR, ROL, ROR, RCL, RCR

; === Test SHL (Shift Left) ===
; SHL shifts bits left, filling with zeros
MOV AL, 0b00001111  ; AL = 15
SHL AL, 1           ; AL = 0b00011110 = 30, CF=0
MOV BL, AL          ; Save to BL

MOV AL, 0b10000001  ; AL = 129
SHL AL, 1           ; AL = 0b00000010 = 2, CF=1 (bit shifted out)
MOV BH, AL          ; Save to BH

; Test 16-bit SHL
MOV AX, 0x0F0F      ; AX = 3855
SHL AX, 1           ; AX = 0x1E1E = 7710, CF=0
MOV CX, AX          ; Save to CX

; === Test SHR (Shift Right Logical) ===
; SHR shifts bits right, filling with zeros
MOV AL, 0b11110000  ; AL = 240
SHR AL, 1           ; AL = 0b01111000 = 120, CF=0
MOV DL, AL          ; Save to DL

MOV AL, 0b00001111  ; AL = 15
SHR AL, 1           ; AL = 0b00000111 = 7, CF=1 (bit shifted out)
MOV DH, AL          ; Save to DH

; === Test SAR (Shift Right Arithmetic) ===
; SAR shifts bits right, preserving sign bit
MOV AL, 0b11110000  ; AL = -16 (signed)
SAR AL, 1           ; AL = 0b11111000 = -8 (sign preserved), CF=0

MOV AL, 0b00001110  ; AL = 14 (positive)
SAR AL, 1           ; AL = 0b00000111 = 7, CF=0

; === Test ROL (Rotate Left) ===
; ROL rotates bits left, bit 7 goes to bit 0 and CF
MOV AL, 0b10000001  ; AL = 129
ROL AL, 1           ; AL = 0b00000011 = 3, CF=1 (bit rotated from MSB)
MOV SI, 0           ; Clear SI
MOV BYTE [SI], AL   ; Can't directly access, so save result via register
MOV SI, AX          ; SI will have rotated result in low byte

; === Test ROR (Rotate Right) ===
; ROR rotates bits right, bit 0 goes to bit 7 and CF
MOV AL, 0b10000001  ; AL = 129
ROR AL, 1           ; AL = 0b11000000 = 192, CF=1 (bit rotated from LSB)
MOV DI, 0           ; Clear DI
MOV DI, AX          ; DI will have rotated result in low byte

; === Test multiple shifts ===
MOV CL, 3           ; Shift count in CL
MOV AL, 0b00000001  ; AL = 1
SHL AL, CL          ; AL = 0b00001000 = 8 (shifted left 3 times)

MOV AL, 0b10000000  ; AL = 128
SHR AL, CL          ; AL = 0b00010000 = 16 (shifted right 3 times)

; === Test 16-bit rotates ===
MOV AX, 0x8001      ; AX = 32769
ROL AX, 1           ; AX = 0x0003, CF=1
MOV BX, AX          ; Save to BX

MOV AX, 0x8001      ; AX = 32769
ROR AX, 1           ; AX = 0xC000, CF=1
MOV DX, AX          ; Save to DX

; === Test edge cases ===
; Shift by 0 (should do nothing)
MOV AL, 0x55
MOV CL, 0
SHL AL, CL          ; AL should remain 0x55

; Large shift count (masked to 5 bits)
MOV AL, 0xFF
MOV CL, 32          ; Will be masked to 0
SHL AL, CL          ; AL should remain 0xFF

HLT

; Expected final register state (key values):
;   BL = 30 (15 << 1)
;   BH = 2  (129 << 1, with overflow)
;   CX = 0x1E1E (0x0F0F << 1)
;   DL = 120 (240 >> 1)
;   DH = 7  (15 >> 1, with carry)
;   BX = 0x0003 (0x8001 rotated left)
;   DX = 0xC000 (0x8001 rotated right)
;
; Notes:
; - SHL/SAL are identical operations
; - Carry flag captures the last bit shifted/rotated out
; - Overflow flag is set only for single-bit shifts
; - SAR preserves the sign bit (MSB)
; - Rotate operations move bits in a circle, including through CF for RCL/RCR
