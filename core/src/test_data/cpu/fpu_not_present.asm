; fpu_not_present.asm
;
; Verifies that no 8087 math coprocessor is present using two independent methods:
;
;   Method 1 - BIOS equipment list (INT 11h):
;     Bit 1 of the returned word must be clear (no coprocessor installed).
;
;   Method 2 - Hardware probe (FNINIT + FNSTSW):
;     Pre-fill AX with 0xFFFF, issue FNINIT then FNSTSW AX.
;     Without an 8087 both instructions are NOPs, so AX stays non-zero.
;     With an 8087 present FNSTSW would store 0x0000, so AX would be zero.
;
; Exit codes:
;   0x00 = PASS: both methods agree – no coprocessor detected
;   0x01 = FAIL: INT 11h reports coprocessor present (unexpected)
;   0x02 = FAIL: FNINIT/FNSTSW reports coprocessor present (unexpected)

[CPU 8086]
[ORG 0x100]

section .text
start:
    ; --- Method 1: BIOS equipment list ---
    int 0x11            ; AX = equipment list word
    test ax, 0x0002     ; bit 1 = math coprocessor installed
    jnz .fail_equip     ; set = coprocessor present (unexpected)

    ; --- Method 2: hardware probe ---
    mov ax, 0xFFFF      ; pre-fill AX; stays non-zero if no 8087
    fninit
    db 0xDF, 0xE0       ; fnstsw ax
    test ax, ax
    jz .fail_hw         ; zero = coprocessor responded (unexpected)

    ; Both methods agree: no coprocessor
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

.fail_equip:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

.fail_hw:
    mov ah, 0x4C
    mov al, 0x02
    int 0x21
