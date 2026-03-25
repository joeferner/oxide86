; UART MCR loopback test
; Mimics the loopback detection sequence used by real DOS software:
;   1. Enable loopback mode via MCR bit 4
;   2. Wait for THRE (LSR bit 5) before each write
;   3. Write a byte to THR
;   4. Poll LSR for DR (bit 0), timeout after 0xFFFF reads
;   5. Read byte from RBR and verify it matches
;   6. Test multiple values to rule out coincidence
;   7. Clear loopback mode and exit 0 on success

[CPU 8086]
org 0x0100

COM1_BASE  equ 0x3F8
COM1_LSR   equ 0x3FD   ; Line Status Register (base + 5)
COM1_MCR   equ 0x3FC   ; Modem Control Register (base + 4)

LSR_DR     equ 0x01    ; bit 0: Data Ready (RBR has a byte)
LSR_THRE   equ 0x20    ; bit 5: Transmitter Holding Register Empty

start:
    ; --- Enable MCR loopback mode (bit 4) ---
    mov dx, COM1_MCR
    mov al, 0x10
    out dx, al

    ; --- Test byte 0x00 ---
    mov al, 0x00
    call do_loopback
    test al, al
    jnz fail

    ; --- Test byte 0x55 ---
    mov al, 0x55
    call do_loopback
    test al, al
    jnz fail

    ; --- Test byte 0xAA ---
    mov al, 0xAA
    call do_loopback
    test al, al
    jnz fail

    ; --- Test byte 0xFF ---
    mov al, 0xFF
    call do_loopback
    test al, al
    jnz fail

    ; --- Disable loopback ---
    mov dx, COM1_MCR
    mov al, 0x00
    out dx, al

    ; Success: exit 0
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    ; Failure: exit 1
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

; do_loopback: send byte in AL through loopback and verify the echo
; Input:  AL = byte to send
; Output: AL = 0 on success, 1 on failure (timeout or wrong byte)
; Clobbers: AH, BX, CX, DX
do_loopback:
    mov bl, al          ; save byte to send in BL

    ; Wait for THRE (LSR bit 5) before writing
    mov cx, 0xFFFF
.wait_thre:
    mov dx, COM1_LSR
    in al, dx
    test al, LSR_THRE
    jnz .write
    loop .wait_thre
    mov al, 1           ; timeout waiting for THRE
    ret

.write:
    ; Write byte to THR
    mov dx, COM1_BASE
    mov al, bl
    out dx, al

    ; Poll LSR for DR (bit 0)
    mov cx, 0xFFFF
.poll_dr:
    mov dx, COM1_LSR
    in al, dx
    test al, LSR_DR
    jnz .read
    loop .poll_dr
    mov al, 1           ; timeout: loopback byte never arrived
    ret

.read:
    ; Read from RBR and verify
    mov dx, COM1_BASE
    in al, dx
    cmp al, bl
    jne .mismatch
    mov al, 0           ; success
    ret

.mismatch:
    mov al, 1           ; wrong byte received
    ret
