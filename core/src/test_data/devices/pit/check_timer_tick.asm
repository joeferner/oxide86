; PIT timer test: verify that the BDA timer tick counter increments via INT 8
;
; Strategy:
;   1. Enable interrupts so IRQ 0 (INT 08h) can be delivered.
;   2. Read the initial tick count (low word → BX) via INT 1Ah AH=00h.
;   3. Poll INT 1Ah in a bounded loop until the low word changes.
;   4. Exit 0 on success, exit 1 on timeout.
;
; At the test CPU speed of 8 MHz, cycles_per_irq ≈ 440 000.  Each loop
; iteration costs a handful of instructions (~60-80 cycles total), so a
; timeout of 65 535 iterations gives roughly 4 000 000 cycles—enough for
; ~9 timer ticks—before declaring failure.

[CPU 8086]
org 0x0100          ; .COM file entry point

start:
    sti             ; enable interrupts – allows PIT IRQ 0 (INT 08h) to fire

    ; Read initial timer tick count
    mov ah, 0x00
    int 0x1a        ; CX:DX = ticks since midnight, AL = midnight flag

    mov bx, dx      ; BX = initial low word of tick count (preserved by INT 1Ah)

    mov si, 0xFFFF  ; SI = timeout counter (65 535 iterations)

poll_loop:
    mov ah, 0x00
    int 0x1a        ; DX = current low word of tick count

    cmp dx, bx      ; has the counter changed?
    jne success     ; yes – timer is ticking, test passes

    dec si
    jnz poll_loop   ; keep polling until timeout

    ; Timeout: the timer counter never changed – test fails
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

success:
    ; Timer ticked at least once – test passes
    mov ah, 0x4C
    mov al, 0x00
    int 0x21
