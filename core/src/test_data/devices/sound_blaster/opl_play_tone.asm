; opl_play_tone.asm - Enable an OPL voice via SB port (0x220/0x221) and play a brief tone
;
; Mirrors adlib_play_tone.asm but accessed through the SB's own OPL port pair.
; Used by the Rust ring-buffer test to verify FM synthesis produces non-zero samples.
;
; At 8 MHz, FLUSH_SIZE (128 samples) needs ~23 000 cycles.
; The delay loop below runs ~50 000 cycles to be safe.

[CPU 8086]
org 0x100

SB_BASE equ 0x220

    ; Enable waveform select (reg 0x01 bit 5)
    mov al, 0x01
    mov dx, SB_BASE
    out dx, al
    mov al, 0x20
    mov dx, SB_BASE + 1
    out dx, al

    ; Modulator (slot 0, reg 0x20): EG=1 MULT=1
    mov al, 0x20
    mov dx, SB_BASE
    out dx, al
    mov al, 0x21
    mov dx, SB_BASE + 1
    out dx, al

    ; Modulator total level (reg 0x40): TL=16
    mov al, 0x40
    mov dx, SB_BASE
    out dx, al
    mov al, 0x10
    mov dx, SB_BASE + 1
    out dx, al

    ; Modulator attack/decay (reg 0x60): AR=15 DR=0
    mov al, 0x60
    mov dx, SB_BASE
    out dx, al
    mov al, 0xF0
    mov dx, SB_BASE + 1
    out dx, al

    ; Modulator sustain/release (reg 0x80): SL=0 RR=7
    mov al, 0x80
    mov dx, SB_BASE
    out dx, al
    mov al, 0x07
    mov dx, SB_BASE + 1
    out dx, al

    ; Carrier (slot 3, reg 0x23): EG=1 MULT=1
    mov al, 0x23
    mov dx, SB_BASE
    out dx, al
    mov al, 0x21
    mov dx, SB_BASE + 1
    out dx, al

    ; Carrier total level (reg 0x43): TL=0 (full volume)
    mov al, 0x43
    mov dx, SB_BASE
    out dx, al
    mov al, 0x00
    mov dx, SB_BASE + 1
    out dx, al

    ; Carrier attack/decay (reg 0x63): AR=15 DR=0
    mov al, 0x63
    mov dx, SB_BASE
    out dx, al
    mov al, 0xF0
    mov dx, SB_BASE + 1
    out dx, al

    ; Carrier sustain/release (reg 0x83): SL=0 RR=7
    mov al, 0x83
    mov dx, SB_BASE
    out dx, al
    mov al, 0x07
    mov dx, SB_BASE + 1
    out dx, al

    ; Channel 0 fnum low byte (reg 0xA0): A4 fnum low = 0x44
    mov al, 0xA0
    mov dx, SB_BASE
    out dx, al
    mov al, 0x44
    mov dx, SB_BASE + 1
    out dx, al

    ; Channel 0 key_on + block + fnum high (reg 0xB0): key_on=1 block=4 fnum_hi=2 = 0x32
    mov al, 0xB0
    mov dx, SB_BASE
    out dx, al
    mov al, 0x32
    mov dx, SB_BASE + 1
    out dx, al

    ; Delay: ~50 000 cycles at 8 MHz → ~275 samples generated
    mov cx, 5000
.wait:
    nop
    nop
    nop
    nop
    nop
    loop .wait

    ; Key off
    mov al, 0xB0
    mov dx, SB_BASE
    out dx, al
    mov al, 0x12       ; key_on=0 block=4 fnum_hi=2
    mov dx, SB_BASE + 1
    out dx, al

    mov ah, 0x4C
    xor al, al
    int 0x21
