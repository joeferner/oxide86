; loop_demo.asm - Demonstrates DEC, CMP, and conditional jumps
; This program counts down from 10 to 1 using a loop

; Initialize counter to 10
MOV CL, 10         ; CL = 10

; Loop start
loop_start:
    ; Decrement counter
    DEC CL         ; CL = CL - 1

    ; Compare counter with zero
    CMP CL, 0      ; Compare CL with 0 (sets flags)

    ; Jump if not zero (continue loop)
    JNZ loop_start ; If CL != 0, jump back to loop_start

; Loop done, CL should be 0
HLT

; Expected final register state:
;   CL = 0x00 (0 decimal)
;
; Expected flags:
;   Zero Flag (ZF) = 1 (result is zero)
;   Sign Flag (SF) = 0 (result is positive)
;   Carry Flag (CF) = 0 (no borrow)
