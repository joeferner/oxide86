; Minimal shift/rotate test

; Test basic SHL
MOV AL, 0x01
SHL AL, 1           ; AL should be 0x02

; Test basic SHR
MOV BL, 0x02
SHR BL, 1           ; BL should be 0x01

; Test ROL
MOV CL, 0x80
ROL CL, 1           ; CL should be 0x01, CF=1

; Test ROR
MOV DL, 0x01
ROR DL, 1           ; DL should be 0x80, CF=1

; Test 16-bit ROL
MOV AX, 0x8001
ROL AX, 1           ; AX should be 0x0003, CF=1

; Test 16-bit ROR
MOV BX, 0x8001
ROR BX, 1           ; BX should be 0xC000, CF=1

HLT

; Expected results:
; AL = 0x02
; BL = 0x01
; CL = 0x01
; DL = 0x80
; AX = 0x0003
; BX = 0xC000
; CF = 1 (from last operation)
