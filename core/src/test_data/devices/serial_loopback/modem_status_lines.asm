; Modem status lines test (physical loopback)
;
; Physical loopback plug wiring:
;   RTS (MCR.1) → CTS (MSR.4)
;   DTR (MCR.0) → DSR (MSR.5) + RI (MSR.6) + DCD (MSR.7)
;
; When modem lines change state the corresponding delta bits (MSR.3:0) are set:
;   DCTS (bit 0), DDSR (bit 1), TERI (bit 2, only on RI 1→0), DDCD (bit 3)
;
; Test sequence:
;   1. MCR=0x00 → MSR & 0xF0 == 0x00   (no lines asserted)
;      Delta bits == 0 (no previous change)
;   2. MCR=0x03 (DTR+RTS) → MSR & 0xFB == 0xFB
;      High nibble: CTS+DSR+RI+DCD = 0xF0
;      Delta bits : DCTS+DDSR+DDCD   = 0x0B  (RI rising edge does NOT set TERI)
;      Result after masking bit 2    : 0xFB
;   3. Read MSR again → delta bits cleared → MSR == 0xF0
;   4. MCR=0x00 → lines de-asserted
;      MSR & 0xFB == 0x0B (DCTS+DDSR+DDCD from falling edges; TERI for RI 1→0)
;      Actually TERI IS set this time (RI goes 1→0), so MSR.3:0 = 0x0F
;      but bit 2 is masked → (0xF & ~0x04) | 0 = 0x0B
;      High nibble is 0x00 → MSR & 0xFB = 0x0B
;   5. Read MSR again → delta bits cleared → MSR == 0x00
;
; Exit 0 = pass, Exit 1 = fail.

[CPU 8086]
org 0x0100

COM1_MCR   equ 0x3FC
COM1_MSR   equ 0x3FE

; Delay constant: spin long enough for modem line changes to settle.
; Loopback is instantaneous in emulation but we keep it real-world-safe.
DELAY      equ 0x4000

start:
    ; --- Step 1: MCR=0, expect MSR high nibble == 0 ---
    mov dx, COM1_MCR
    mov al, 0x00
    out dx, al

    call spin_delay

    mov dx, COM1_MSR
    in al, dx
    and al, 0xF0            ; check only modem line bits
    cmp al, 0x00
    jne fail

    ; --- Step 2: MCR=0x03 (DTR+RTS), expect (MSR & 0xFB) == 0xFB ---
    mov dx, COM1_MCR
    mov al, 0x03
    out dx, al

    call spin_delay

    mov dx, COM1_MSR
    in al, dx
    and al, 0xFB            ; mask out TERI (bit 2)
    cmp al, 0xFB
    jne fail

    ; --- Step 3: second read → delta bits cleared, MSR == 0xF0 ---
    mov dx, COM1_MSR
    in al, dx
    cmp al, 0xF0
    jne fail

    ; --- Step 4: MCR=0x00, lines fall → (MSR & 0xFB) == 0x0B ---
    mov dx, COM1_MCR
    mov al, 0x00
    out dx, al

    call spin_delay

    mov dx, COM1_MSR
    in al, dx
    and al, 0xFB            ; mask TERI
    cmp al, 0x0B
    jne fail

    ; --- Step 5: second read → delta bits cleared, MSR == 0x00 ---
    mov dx, COM1_MSR
    in al, dx
    cmp al, 0x00
    jne fail

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

spin_delay:
    mov cx, DELAY
.loop:
    loop .loop
    ret
