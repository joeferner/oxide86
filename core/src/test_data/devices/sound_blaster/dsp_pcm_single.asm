; dsp_pcm_single.asm — 8-bit single-cycle DMA PCM playback
;
; 1. Install IRQ5 handler at INT 0x0D (vector 0x34)
; 2. Reset DSP
; 3. Set time constant for 11025 Hz (TC = 166)
; 4. Fill 256-byte buffer at 0x2000:0x0000 with 0x80 (silence)
; 5. Program DMA channel 1: 256 bytes, READ mode (mem -> device)
; 6. Unmask IRQ5 at PIC
; 7. Issue DSP command 0x14 (single-cycle 8-bit DMA, 255 bytes)
; 8. Enable interrupts; wait for IRQ flag (up to 0xFFFF iterations)
; 9. Exit 0 if IRQ fired, 1 if timeout
;
; DMA1 channel 1 ports: addr=0x02, count=0x03, mode=0x0B, mask=0x0A, page=0x83, flip-flop=0x0C

[CPU 8086]
org 0x100

SB_BASE equ 0x220

start:
    ; --- Install IRQ5 handler (INT 0x0D = 4 * 13 = 0x34) ---
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
    in al, dx          ; consume 0xAA ready byte

    ; --- Set time constant for 11025 Hz mono (TC = 166) ---
    mov dx, SB_BASE + 0xC
    mov al, 0x40
    out dx, al
    mov al, 166
    out dx, al

    ; --- Fill audio buffer at 0x2000:0x0000 with 0x80 (silence) ---
    push es
    mov ax, 0x2000
    mov es, ax
    xor di, di
    mov cx, 256
    mov al, 0x80
    rep stosb
    pop es

    ; --- Program DMA1 channel 1 ---
    ; Mask channel 1 before reprogramming
    mov al, 0x05       ; set-mask bit + channel 1
    out 0x0A, al

    ; Reset flip-flop
    xor al, al
    out 0x0C, al

    ; Base address = 0x0000 (physical 0x20000)
    xor al, al
    out 0x02, al       ; low byte
    out 0x02, al       ; high byte

    ; Page register for channel 1 = 0x02
    mov al, 0x02
    out 0x83, al

    ; Reset flip-flop before count
    xor al, al
    out 0x0C, al

    ; Count = 255 (256 bytes total)
    mov al, 0xFF
    out 0x03, al       ; low byte
    xor al, al
    out 0x03, al       ; high byte

    ; Mode: single-cycle, increment, no-auto-init, READ (mem->device), channel 1 = 0x49
    mov al, 0x49
    out 0x0B, al

    ; Unmask channel 1
    mov al, 0x01       ; clear-mask bit + channel 1
    out 0x0A, al

    ; --- Unmask IRQ5 at PIC ---
    in al, 0x21
    and al, 0xDF       ; clear bit 5
    out 0x21, al

    ; --- Issue DSP single-cycle 8-bit DMA command ---
    mov dx, SB_BASE + 0xC
    mov al, 0x14
    out dx, al
    mov al, 0xFF       ; length lo = 255 (256 bytes total)
    out dx, al
    xor al, al         ; length hi = 0
    out dx, al

    ; --- Enable interrupts and wait for IRQ ---
    sti
    mov cx, 0xFFFF
.wait:
    cmp byte [irq_fired], 1
    je .pass
    loop .wait

    ; Timeout: exit code 1
    mov bl, 0x01
    jmp .exit

.pass:
    xor bl, bl
.exit:
    ; Mask IRQ5 again before exiting
    in al, 0x21
    or al, 0x20
    out 0x21, al
    mov al, bl
    mov ah, 0x4C
    int 0x21

irq_handler:
    mov byte [cs:irq_fired], 1
    ; Acknowledge 8-bit IRQ by reading DSP read-status port
    mov dx, SB_BASE + 0xE
    in al, dx
    ; Non-specific EOI to master PIC
    mov al, 0x20
    out 0x20, al
    iret

irq_fired db 0
