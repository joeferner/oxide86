; RTC alarm INT 4Ah chain test: verify that when IRQ 8 (INT 70h) fires and
; the Alarm Flag is set, the BIOS INT 70h handler chains to INT 4Ah as the
; IBM AT BIOS does.
;
; Setup:
;   - Install a custom INT 4Ah handler that sets alarm_fired=1.
;   - Leave INT 70h pointing at the BIOS (do NOT install our own INT 70h).
;   - Program the alarm to MockClock time (BCD 11:05:30) and enable AIE.
;   - Spin until alarm_fired or timeout.
;
; If the BIOS INT 70h handler correctly chains to INT 4Ah, alarm_fired will
; be set. If it forgets to call INT 4Ah, we timeout and exit 1.
;
; Exit 0 on success, exit 1 on timeout.

[CPU 8086]
org 0x0100

    jmp start

alarm_fired: db 0

start:
    cli

    ; Install INT 4Ah handler — leave INT 70h pointing at the BIOS.
    xor ax, ax
    mov es, ax
    mov word [es:0x4A*4],   int4a_handler
    mov word [es:0x4A*4+2], cs

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

    ; Timed out — INT 4Ah was never called
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

success:
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

int4a_handler:
    ; Called by BIOS INT 70h handler when the alarm fires (AF bit in Status C).
    ; No acknowledgement needed here — BIOS already read Status C and sent EOIs.
    mov byte [cs:alarm_fired], 1
    iret
