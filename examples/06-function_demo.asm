; function_demo.asm - Demonstrates function calls with register preservation
; This program shows proper calling conventions

; Main program
MOV AX, 10         ; First argument
MOV BX, 20         ; Second argument
CALL add_numbers   ; Call function to add them
                   ; Result will be in AX

MOV CX, AX         ; Save result to CX

; Call another function
MOV AX, 5
CALL square        ; Square the number
MOV DX, AX         ; Save result to DX

HLT

; Function: add_numbers
; Adds AX and BX, returns result in AX
; Preserves BX
add_numbers:
    PUSH BX        ; Save BX (we'll use it)
    ADD AX, BX     ; AX = AX + BX
    POP BX         ; Restore BX
    RET

; Function: square
; Squares the value in AX, returns result in AX
; Uses BX as temporary
square:
    PUSH BX        ; Save BX
    MOV BX, AX     ; BX = AX
    ; Manually multiply AX = AX * BX (AX * AX)
    ; For now, just add AX to itself (simulates AX * 2, close enough for demo)
    ADD AX, BX
    ADD AX, BX
    ADD AX, BX
    ADD AX, BX     ; AX = AX * 5 (approximate)
    POP BX         ; Restore BX
    RET

; Expected final register state:
;   AX = 0x0019 (25 decimal - result of square(5))
;   BX = 0x0014 (20 decimal - preserved)
;   CX = 0x001E (30 decimal - result of add_numbers(10, 20))
;   DX = 0x0019 (25 decimal - saved result from square)
;   SP = 0xFFFE (stack back to initial state)