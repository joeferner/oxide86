; cdrom_nop.asm — SoundBlaster CD-ROM NOP command (0x00) detection test
;
; Verifies that the Panasonic CD-ROM interface within SoundBlaster responds to
; the NOP/presence-check command (0x00) and returns the expected 2-byte signature
; [0xAA, 0x55].
;
; Protocol:
;   1. Write command 0x00 to base+0 (0x230)
;   2. Poll base+1 until bit 2 (busy) is clear
;   3. Read byte from base+0 — must be 0xAA
;   4. Read byte from base+0 — must be 0x55
;
; Exit codes: 0=pass, 1=timeout waiting for ready, 2=first byte != 0xAA, 3=second byte != 0x55

[CPU 8086]
org 0x100

SB_CD_BASE equ 0x230

start:
    ; Send NOP command (0x00) to base+0
    mov dx, SB_CD_BASE
    xor al, al
    out dx, al

    ; Poll base+1 until bit 2 (busy) is clear
    mov cx, 5000
.poll:
    mov dx, SB_CD_BASE + 1
    in al, dx
    test al, 0x04
    jz .ready
    loop .poll

    ; Timeout
    mov al, 0x01
    jmp .exit

.ready:
    ; Read first result byte from base+0 — expect 0xAA
    mov dx, SB_CD_BASE
    in al, dx
    cmp al, 0xAA
    je .check_second
    mov al, 0x02
    jmp .exit

.check_second:
    ; Read second result byte from base+0 — expect 0x55
    mov dx, SB_CD_BASE
    in al, dx
    cmp al, 0x55
    je .pass
    mov al, 0x03
    jmp .exit

.pass:
    xor al, al
.exit:
    mov ah, 0x4C
    int 0x21
