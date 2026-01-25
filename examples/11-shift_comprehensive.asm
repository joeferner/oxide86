; Comprehensive shift/rotate test with expected values

; === SHL Tests ===
MOV AL, 0x0F
SHL AL, 1           ; Expected: AL=0x1E, CF=0

MOV BL, 0x80
SHL BL, 1           ; Expected: BL=0x00, CF=1

MOV AX, 0x4000
SHL AX, 1           ; Expected: AX=0x8000, CF=0, OF=1 (sign changed)

; === SHR Tests ===
MOV CL, 0xF0
SHR CL, 1           ; Expected: CL=0x78, CF=0

MOV DL, 0x0F
SHR DL, 1           ; Expected: DL=0x07, CF=1

; === SAR Tests ===
MOV AL, 0xF0        ; Negative number (sign bit set)
SAR AL, 1           ; Expected: AL=0xF8, CF=0 (sign preserved)

MOV BL, 0x0E        ; Positive number
SAR BL, 1           ; Expected: BL=0x07, CF=0

; === ROL Tests ===
MOV AL, 0x80
ROL AL, 1           ; Expected: AL=0x01, CF=1

MOV BX, 0x8000
ROL BX, 1           ; Expected: BX=0x0001, CF=1

; === ROR Tests ===
MOV CL, 0x01
ROR CL, 1           ; Expected: CL=0x80, CF=1

MOV DX, 0x0001
ROR DX, 1           ; Expected: DX=0x8000, CF=1

; === Shift by CL register ===
MOV CL, 4
MOV AL, 0x01
SHL AL, CL          ; Expected: AL=0x10 (shifted left 4 times)

MOV BL, 0x80
SHR BL, CL          ; Expected: BL=0x08 (shifted right 4 times)

; === RCL/RCR Tests ===
; Setup: Clear carry
MOV AL, 0x00
SHL AL, 1           ; CF=0

MOV AL, 0x80
RCL AL, 1           ; Expected: AL=0x00, CF=1 (MSB rotated to CF, CF(0) rotated to LSB)

; Now CF=1 from previous operation
MOV BL, 0x01
RCR BL, 1           ; Expected: BL=0x80, CF=1 (LSB rotated to CF, CF(1) rotated to MSB)

; === Edge case: Shift by 0 ===
MOV CL, 0
MOV AL, 0x55
SHL AL, CL          ; Expected: AL=0x55 (no change)

HLT

; Final expected state (tracking key registers):
; AL = 0x55 (from last test)
; BL = 0x80 (from RCR)
; CL = 0x00 (shift count)
; DL = 0x07 (from earlier SHR)
; BX = 0x0080 (BH from earlier, BL from RCR)
; DX = 0x8007 (DH from earlier, DL from SHR)
; CF = Should vary based on last operation
