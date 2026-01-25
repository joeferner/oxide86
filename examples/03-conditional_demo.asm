; conditional_demo.asm - Demonstrates various conditional jumps
; This program tests different comparison scenarios

; Test 1: Equal comparison
MOV AL, 5
MOV BL, 5
CMP AL, BL         ; 5 == 5, sets ZF=1
JE equal_found     ; Should jump (they are equal)
MOV CL, 0xFF       ; Should NOT execute
equal_found:
MOV CL, 0x01       ; CL = 1 (confirms JE worked)

; Test 2: Not equal comparison
MOV AL, 7
MOV BL, 3
CMP AL, BL         ; 7 != 3, sets ZF=0
JNE not_equal_found ; Should jump (they are not equal)
MOV DL, 0xFF       ; Should NOT execute
not_equal_found:
MOV DL, 0x02       ; DL = 2 (confirms JNE worked)

; Test 3: Greater than (unsigned)
MOV AL, 10
MOV BL, 5
CMP AL, BL         ; 10 > 5
JA above_found     ; Should jump (10 is above 5)
MOV AH, 0xFF       ; Should NOT execute
above_found:
MOV AH, 0x03       ; AH = 3 (confirms JA worked)

; Test 4: Less than (unsigned)
MOV AL, 3
MOV BL, 8
CMP AL, BL         ; 3 < 8
JB below_found     ; Should jump (3 is below 8)
MOV BH, 0xFF       ; Should NOT execute
below_found:
MOV BH, 0x04       ; BH = 4 (confirms JB worked)

HLT

; Expected final register state:
;   AL = 0x03 (3 decimal)
;   AH = 0x03 (3 decimal)
;   BL = 0x08 (8 decimal)
;   BH = 0x04 (4 decimal)
;   CL = 0x01 (1 decimal)
;   DL = 0x02 (2 decimal)
