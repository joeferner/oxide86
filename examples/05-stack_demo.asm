; stack_demo.asm - Demonstrates PUSH, POP, CALL, and RET
; This program shows basic stack operations and function calls

; Test PUSH and POP
MOV AX, 0x1234     ; AX = 0x1234
MOV BX, 0x5678     ; BX = 0x5678
PUSH AX            ; Push AX onto stack
PUSH BX            ; Push BX onto stack
POP CX             ; Pop into CX (should be 0x5678)
POP DX             ; Pop into DX (should be 0x1234)

; Test PUSH immediate
PUSH 0xABCD        ; Push immediate value

; Call a function
CALL add_function

; Pop the immediate we pushed earlier
POP AX             ; AX = 0xABCD

HLT

; Function that adds 10 to AL
add_function:
    MOV AL, 5      ; AL = 5
    ADD AL, 10     ; AL = 15
    RET

; Expected final register state:
;   AX = 0xABCD (popped immediate - overwrites AL from function)
;   BX = 0x5678
;   CX = 0x5678 (popped from stack)
;   DX = 0x1234 (popped from stack)
;   SP = 0xFFFE (stack should be back to initial state)