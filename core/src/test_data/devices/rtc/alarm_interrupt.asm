; RTC alarm interrupt test: verify that IRQ 8 (INT 70h) fires when the alarm
; time matches the current RTC time.
;
; The test MockClock is fixed at 11:05:30, so we program the alarm to that
; time (BCD 0x11:0x05:0x30). Enabling AIE in Status Register B (0x0B)
; arms the interrupt. The INT 70h handler acknowledges Status Register C,
; sends EOI to both PICs, and sets a flag. A polling loop with a timeout
; detects whether the interrupt fired.
;
; Exit 0 on success (interrupt fired), exit 1 on timeout.

[CPU 8086]
org 0x0100

    jmp start

alarm_fired: db 0

start:
    cli

    ; Install INT 70h handler in the IVT
    xor ax, ax
    mov es, ax
    mov word [es:0x70*4],   int70_handler
    mov word [es:0x70*4+2], cs

    ; Set seconds alarm register (0x01) = 0x30 (BCD 30)
    mov al, 0x81
    out 0x70, al
    mov al, 0x30
    out 0x71, al

    ; Set minutes alarm register (0x03) = 0x05 (BCD 5)
    mov al, 0x83
    out 0x70, al
    mov al, 0x05
    out 0x71, al

    ; Set hours alarm register (0x05) = 0x11 (BCD 11)
    mov al, 0x85
    out 0x70, al
    mov al, 0x11
    out 0x71, al

    ; Status Register B (0x0B): bit 5 = AIE (alarm interrupt enable), bit 1 = 24h mode
    mov al, 0x8B
    out 0x70, al
    mov al, 0x22
    out 0x71, al

    sti

    mov si, 0xFFFF      ; timeout counter (~65 535 poll iterations)

poll_loop:
    cmp byte [alarm_fired], 1
    je  success
    dec si
    jnz poll_loop

    ; Timed out — interrupt never fired
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

success:
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

int70_handler:
    ; Acknowledge the interrupt: read Status Register C (0x0C), clears AF/IRQF
    mov al, 0x8C
    out 0x70, al
    in  al, 0x71

    ; Send EOI to PIC2, then cascade EOI to PIC1
    mov al, 0x20
    out 0xA0, al
    out 0x20, al

    mov byte [cs:alarm_fired], 1
    iret
