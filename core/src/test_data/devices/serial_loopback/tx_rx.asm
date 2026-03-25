; Serial loopback TX→RX test
; Writes several bytes to COM1 THR (with a physical loopback device attached)
; and reads them back from RBR.  Verifies each byte round-trips correctly.
;
; Exit 0 = pass, Exit 1 = fail.

[CPU 8086]
org 0x0100

COM1_BASE  equ 0x3F8
COM1_LSR   equ 0x3FD

LSR_DR     equ 0x01    ; Data Ready
LSR_THRE   equ 0x20    ; Transmitter Holding Register Empty

start:
    ; Test a handful of byte values
    mov al, 0x00
    call do_loopback
    test al, al
    jnz fail

    mov al, 0x55
    call do_loopback
    test al, al
    jnz fail

    mov al, 0xAA
    call do_loopback
    test al, al
    jnz fail

    mov al, 0xFF
    call do_loopback
    test al, al
    jnz fail

    mov al, 0x42
    call do_loopback
    test al, al
    jnz fail

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

; do_loopback: send byte in AL through physical loopback and verify the echo.
; Input:  AL = byte to send
; Output: AL = 0 success, 1 failure
; Clobbers: AH, BL, CX, DX
do_loopback:
    mov bl, al          ; save expected byte

    ; Wait for THRE
    mov cx, 0xFFFF
.wait_thre:
    mov dx, COM1_LSR
    in al, dx
    test al, LSR_THRE
    jnz .write
    loop .wait_thre
    mov al, 1
    ret

.write:
    mov dx, COM1_BASE
    mov al, bl
    out dx, al

    ; Poll for DR
    mov cx, 0xFFFF
.poll_dr:
    mov dx, COM1_LSR
    in al, dx
    test al, LSR_DR
    jnz .read
    loop .poll_dr
    mov al, 1
    ret

.read:
    mov dx, COM1_BASE
    in al, dx
    cmp al, bl
    jne .mismatch
    mov al, 0
    ret

.mismatch:
    mov al, 1
    ret
