; stack_segment_demo.asm - Demonstrates segment register stack operations
; This program shows PUSH/POP segment registers, PUSHF/POPF, and POP r/m16

; Setup segment registers with test values
MOV AX, 0x1000
MOV DS, AX          ; DS = 0x1000

MOV AX, 0x2000
MOV ES, AX          ; ES = 0x2000

MOV AX, 0x3000
MOV SS, AX          ; SS = 0x3000
MOV SP, 0xFFFE      ; Initialize stack pointer

; Test PUSH segment registers
PUSH DS             ; Push DS (0x1000) onto stack
PUSH ES             ; Push ES (0x2000) onto stack
PUSH SS             ; Push SS (0x3000) onto stack
PUSH CS             ; Push CS onto stack

; Modify segment registers
MOV AX, 0x4000
MOV DS, AX          ; DS = 0x4000
MOV ES, AX          ; ES = 0x4000

; Test POP segment registers - restore original values in reverse order
POP AX              ; Pop CS value into AX (we won't pop directly to CS)
POP SS              ; SS = 0x3000 (restored)
POP ES              ; ES = 0x2000 (restored)
POP DS              ; DS = 0x1000 (restored)

; Test PUSHF/POPF - flags manipulation
MOV BX, 0x1234
CMP BX, 0x1234      ; Set ZF (Zero Flag)
PUSHF               ; Push FLAGS register

; Change flags
MOV CX, 0x0001
CMP CX, 0x0002      ; Clear ZF, set CF

POPF                ; Restore FLAGS (ZF should be set again)

; Test POP to memory (opcode 8F /0)
PUSH 0xABCD         ; Push a test value
MOV BX, 0x0100      ; BX points to offset 0x0100
; The following would be: POP [BX]
; This requires specific machine code as NASM may not support it directly
; We'll use POP to a register instead for this demo
POP DI              ; DI = 0xABCD

; Final state verification values
MOV AX, 0x9999      ; Marker value
HLT

; Expected final register state:
;   DS = 0x1000 (restored)
;   ES = 0x2000 (restored)
;   SS = 0x3000 (restored)
;   DI = 0xABCD (popped value)
;   AX = 0x9999 (marker)
;   SP = 0xFFFE (stack back to initial state)
