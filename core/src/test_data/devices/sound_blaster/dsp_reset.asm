; dsp_reset.asm — DSP reset and version check
;
; Standard SB16 detection sequence:
;   1. Write 0x01 to reset port (base+6), wait, write 0x00
;   2. Poll read-status port (base+E) until bit 7 is set
;   3. Read byte from base+A — must be 0xAA (DSP ready)
;   4. Send command 0xE1 (version), read two bytes
;   5. Verify major=0x04, minor=0x05
;
; Exit codes: 0=pass, 1=DSP ready byte was not 0xAA, 2=wrong major, 3=wrong minor

[CPU 8086]
org 0x100

SB_BASE equ 0x220

start:
    ; Reset DSP: write 1
    mov dx, SB_BASE + 6
    mov al, 0x01
    out dx, al

    ; Short delay
    mov cx, 100
.delay1:
    nop
    loop .delay1

    ; Write 0 to complete reset
    mov al, 0x00
    out dx, al

    ; Poll base+E bit 7 until data ready (up to ~2000 iterations)
    mov cx, 2000
.poll_ready:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .read_aa
    loop .poll_ready

.read_aa:
    ; Read the ready byte from base+A
    mov dx, SB_BASE + 0xA
    in al, dx
    cmp al, 0xAA
    je .send_version
    mov al, 0x01        ; fail: wrong ready byte
    jmp .exit

.send_version:
    ; Send version command 0xE1
    mov dx, SB_BASE + 0xC
    mov al, 0xE1
    out dx, al

    ; Poll and read major version
    mov cx, 2000
.poll_major:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .read_major
    loop .poll_major
.read_major:
    mov dx, SB_BASE + 0xA
    in al, dx
    cmp al, 0x04
    je .read_minor_byte
    mov al, 0x02        ; fail: wrong major
    jmp .exit

.read_minor_byte:
    ; Poll and read minor version
    mov cx, 2000
.poll_minor:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .read_minor
    loop .poll_minor
.read_minor:
    mov dx, SB_BASE + 0xA
    in al, dx
    cmp al, 0x05
    je .pass
    mov al, 0x03        ; fail: wrong minor
    jmp .exit

.pass:
    xor al, al
.exit:
    mov ah, 0x4C
    int 0x21
