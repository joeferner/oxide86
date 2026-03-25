; UART register bit-width test
; Verifies that IER and MCR only preserve their implemented bits:
;   IER bits 3:0  (mask 0x0F) — write 0x55, expect readback 0x05
;   MCR bits 4:0  (mask 0x1F) — write 0x55, expect readback 0x15
; Also verifies LCR round-trips all 8 bits (it is fully implemented).

[CPU 8086]
org 0x0100

COM1_BASE  equ 0x3F8
COM1_IER   equ 0x3F9   ; IER (DLAB=0)
COM1_LCR   equ 0x3FB   ; Line Control Register
COM1_MCR   equ 0x3FC   ; Modem Control Register

start:
    ; --- IER scratch test: write 0x55, expect 0x05 (bits 3:0 only) ---
    ; Save IER first (DLAB must be 0)
    mov dx, COM1_LCR
    in  al, dx
    and al, 0x7F        ; clear DLAB
    out dx, al

    mov dx, COM1_IER
    in  al, dx
    mov bl, al          ; save original IER

    mov al, 0x55
    out dx, al          ; write pattern

    in  al, dx          ; read back
    cmp al, 0x05        ; expect 0x55 & 0x0F = 0x05
    jne fail

    ; Restore IER
    mov al, bl
    out dx, al

    ; --- MCR scratch test: write 0x55, expect 0x15 (bits 4:0 only) ---
    mov dx, COM1_MCR
    in  al, dx
    mov bh, al          ; save original MCR

    mov al, 0x55
    out dx, al          ; write pattern

    in  al, dx          ; read back
    cmp al, 0x15        ; expect 0x55 & 0x1F = 0x15
    jne fail

    ; Restore MCR
    mov al, bh
    out dx, al

    ; --- LCR round-trip: write 0x55, expect 0x55 (all 8 bits) ---
    mov dx, COM1_LCR
    in  al, dx
    mov cl, al          ; save original LCR

    mov al, 0x55
    out dx, al

    in  al, dx
    cmp al, 0x55        ; LCR is fully implemented — all bits preserved
    jne fail

    ; Restore LCR
    mov al, cl
    out dx, al

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
