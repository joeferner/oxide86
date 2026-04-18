; dsp_pcm_samples.asm — 8-bit DMA PCM playback with sawtooth waveform
;
; Identical flow to dsp_pcm_single.asm but fills the DMA buffer with a
; sawtooth waveform (0x00 through 0xFF) instead of silence, so the Rust
; test can verify that non-zero samples appear in the PCM ring buffer.
;
; Exit: 0 if IRQ fired (DMA complete), 1 if timeout

[CPU 8086]
org 0x100

SB_BASE equ 0x220

start:
    ; --- Install IRQ5 handler (INT 0x0D) ---
    push es
    xor ax, ax
    mov es, ax
    mov word [es:0x34], irq_handler
    mov word [es:0x36], cs
    pop es

    ; --- Reset DSP ---
    mov dx, SB_BASE + 6
    mov al, 0x01
    out dx, al
    mov cx, 100
.rst:
    nop
    loop .rst
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
    in al, dx

    ; --- Fill buffer at 0x2000:0x0000 with sawtooth 0x00..0xFF ---
    push es
    mov ax, 0x2000
    mov es, ax
    xor di, di
    xor al, al
    mov cx, 256
.fill:
    stosb
    inc al
    loop .fill
    pop es

    ; --- Program DMA1 channel 1 ---
    mov al, 0x05
    out 0x0A, al       ; mask channel 1
    xor al, al
    out 0x0C, al       ; flip-flop reset
    xor al, al
    out 0x02, al       ; address low
    out 0x02, al       ; address high
    mov al, 0x02
    out 0x83, al       ; page register
    xor al, al
    out 0x0C, al       ; flip-flop reset
    mov al, 0xFF
    out 0x03, al       ; count low
    xor al, al
    out 0x03, al       ; count high
    mov al, 0x49
    out 0x0B, al       ; mode: single-cycle, READ, channel 1
    mov al, 0x01
    out 0x0A, al       ; unmask channel 1

    ; --- Unmask IRQ5 ---
    in al, 0x21
    and al, 0xDF
    out 0x21, al

    ; --- Issue DSP command 0x14 (256 bytes) ---
    mov dx, SB_BASE + 0xC
    mov al, 0x14
    out dx, al
    mov al, 0xFF
    out dx, al
    xor al, al
    out dx, al

    ; --- Wait for IRQ ---
    sti
    mov cx, 0xFFFF
.wait:
    cmp byte [irq_fired], 1
    je .pass
    loop .wait

    mov bl, 0x01
    jmp .exit

.pass:
    xor bl, bl
.exit:
    in al, 0x21
    or al, 0x20
    out 0x21, al
    mov al, bl
    mov ah, 0x4C
    int 0x21

irq_handler:
    mov byte [cs:irq_fired], 1
    mov dx, SB_BASE + 0xE
    in al, dx
    mov al, 0x20
    out 0x20, al
    iret

irq_fired db 0
