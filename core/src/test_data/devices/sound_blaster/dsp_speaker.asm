; dsp_speaker.asm — Speaker on/off commands and status readback
;
; Exit codes: 0=pass, 1=wrong initial status, 2=wrong on status, 3=wrong off status

[CPU 8086]
org 0x100

SB_BASE equ 0x220

%macro sb_cmd 1
    mov dx, SB_BASE + 0xC
    mov al, %1
    out dx, al
%endmacro

%macro sb_read 0
    mov cx, 2000
%%poll:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz %%done
    loop %%poll
%%done:
    mov dx, SB_BASE + 0xA
    in al, dx
%endmacro

start:
    ; Reset DSP first
    mov dx, SB_BASE + 6
    mov al, 0x01
    out dx, al
    mov cx, 100
.r: nop
    loop .r
    xor al, al
    out dx, al
    mov cx, 2000
.pw:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .drain
    loop .pw
.drain:
    mov dx, SB_BASE + 0xA
    in al, dx          ; consume 0xAA ready byte

    ; Check default speaker status (off = 0x00)
    sb_cmd 0xD8
    sb_read
    cmp al, 0x00
    je .turn_on
    mov al, 0x01
    jmp .exit

.turn_on:
    ; Speaker on
    sb_cmd 0xD1
    sb_cmd 0xD8
    sb_read
    cmp al, 0xFF
    je .turn_off
    mov al, 0x02
    jmp .exit

.turn_off:
    ; Speaker off
    sb_cmd 0xD3
    sb_cmd 0xD8
    sb_read
    cmp al, 0x00
    je .pass
    mov al, 0x03
    jmp .exit

.pass:
    xor al, al
.exit:
    mov ah, 0x4C
    int 0x21
