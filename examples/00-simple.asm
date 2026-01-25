; Simple test program for emu86
; This loads some values into registers and halts

    mov ax, 0x1234      ; Load 0x1234 into AX
    mov bx, 0x5678      ; Load 0x5678 into BX
    mov cl, 0x42        ; Load 0x42 into CL
    hlt                 ; Halt the CPU
