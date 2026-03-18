; fpu_present.asm
;
; Verifies that an 8087 math coprocessor IS present using two independent methods:
;
;   Method 1 - BIOS equipment list (INT 11h):
;     Bit 1 of the returned word must be set (coprocessor installed).
;
;   Method 2 - Hardware probe (FNINIT + FNSTSW):
;     Pre-fill AX with 0xFFFF, issue FNINIT then FNSTSW AX.
;     With an 8087 present FNINIT resets the coprocessor status word to 0x0000,
;     and FNSTSW stores that value into AX, so AX becomes zero.
;     Without an 8087 both instructions are NOPs and AX stays non-zero.
;
; Exit codes:
;   0x00 = PASS: both methods agree – coprocessor detected
;   0x01 = FAIL: INT 11h reports no coprocessor (unexpected)
;   0x02 = FAIL: FNINIT/FNSTSW reports no coprocessor (unexpected)

[CPU 8086]
[ORG 0x100]

section .text
start:
    ; --- Method 1: BIOS equipment list ---
    int 0x11            ; AX = equipment list word
    test ax, 0x0002     ; bit 1 = math coprocessor installed
    jz .fail_equip      ; clear = no coprocessor (unexpected)

    ; --- Method 2: hardware probe ---
    mov ax, 0xFFFF      ; pre-fill AX; stays non-zero if no 8087
    fninit
    db 0xDF, 0xE0       ; fnstsw ax
    test ax, ax
    jnz .fail_hw        ; non-zero = coprocessor did not respond (unexpected)

    ; Both methods agree: coprocessor present
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
