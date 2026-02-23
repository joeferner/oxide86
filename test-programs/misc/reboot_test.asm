; reboot_test.asm - Tests warm reboot via JMP FAR FFFF:0000
; The standard DOS/BIOS warm-boot sequence:
;   1. Write 0x1234 to 0040:0072 (warm-boot flag, tells POST to skip memory test)
;   2. JMP FAR 0xFFFF:0000  (the 8086 reset vector)
;
; Expected behavior: emulator detects the jump to FFFF:0000 and triggers reset(),
; which re-runs the boot sequence from whatever drive was originally used.

org 0x100

start:
    ; Print message
    mov  dx, msg_before
    mov  ah, 0x09
    int  0x21

    ; Set warm-boot flag at 0040:0072 = 0x1234
    mov  ax, 0x0040
    mov  es, ax
    mov  word [es:0x0072], 0x1234

    ; Print "jumping..." message
    mov  dx, msg_jump
    mov  ah, 0x09
    int  0x21

    ; Jump to reset vector FFFF:0000
    jmp  0xFFFF:0x0000

msg_before  db 'Reboot test: writing warm-boot flag...', 0x0D, 0x0A, '$'
msg_jump    db 'Jumping to FFFF:0000 (reset vector)...', 0x0D, 0x0A, '$'
