; dsp_dma_detect.asm — SB DSP 0xE2 DMA identification probe
;
; Replicates the card-detection sequence used by the game's func_sb_dma_detect
; (188F:7F5A).  The DSP command 0xE2 triggers a 1-byte DMA WRITE (device→memory)
; transfer; the result byte for parameter 0xA5 must be 0xDD (= 0xA5 XOR 0x78).
;
; Steps:
;   1. Reset DSP; verify 0xAA ready byte
;   2. Install IRQ5 handler at INT 0x0D
;   3. Program DMA channel 1: 1-byte WRITE to buffer at 0x2000:0x0000 (phys 0x20000)
;   4. Unmask IRQ5 at PIC
;   5. Send DSP command 0xE2 + 0xA5; enable interrupts; wait for IRQ (up to 0xFFFF)
;   6. Verify result byte at 0x2000:0x0000 equals 0xDD
;
; Exit codes: 0=pass, 1=DSP ready byte not 0xAA, 2=IRQ timeout, 3=wrong DMA result

[CPU 8086]
org 0x100

SB_BASE equ 0x220
DMA_RESULT_SEG equ 0x2000

start:
    ; --- Reset DSP ---
    mov dx, SB_BASE + 6
    mov al, 0x01
    out dx, al
    mov cx, 100
.rst_delay:
    nop
    loop .rst_delay
    xor al, al
    out dx, al

    ; Poll base+E for data ready
    mov cx, 2000
.poll_rst:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .read_aa
    loop .poll_rst

.read_aa:
    mov dx, SB_BASE + 0xA
    in al, dx
    cmp al, 0xAA
    je .install_irq
    mov al, 0x01        ; fail: bad ready byte
    jmp .exit

    ; --- Install IRQ5 handler at INT 0x0D ---
.install_irq:
    push es
    xor ax, ax
    mov es, ax
    mov word [es:0x34], irq_handler
    mov word [es:0x36], cs
    pop es

    ; --- Zero the result buffer ---
    push es
    mov ax, DMA_RESULT_SEG
    mov es, ax
    mov byte [es:0], 0x00
    pop es

    ; --- Program DMA channel 1 (device→memory WRITE, 1 byte at 0x2000:0x0000) ---
    ; Mask channel 1 before reprogramming
    mov al, 0x05        ; set-mask + channel 1
    out 0x0A, al

    ; Clear byte-pointer flip-flop
    xor al, al
    out 0x0C, al

    ; Base address = 0x0000 within page
    xor al, al
    out 0x02, al        ; low byte
    out 0x02, al        ; high byte

    ; Page register for channel 1 = 0x02 (physical 0x20000)
    mov al, 0x02
    out 0x83, al

    ; Clear flip-flop before count
    xor al, al
    out 0x0C, al

    ; Count = 0 (1 byte total)
    xor al, al
    out 0x03, al        ; low byte
    out 0x03, al        ; high byte

    ; Mode = 0x45: single-cycle, increment, no-auto-init, WRITE (device→mem), channel 1
    mov al, 0x45
    out 0x0B, al

    ; Unmask channel 1
    mov al, 0x01        ; clear-mask + channel 1
    out 0x0A, al

    ; --- Unmask IRQ5 at PIC ---
    in al, 0x21
    and al, 0xDF        ; clear bit 5
    out 0x21, al

    ; --- Send DSP command 0xE2 + parameter 0xA5 ---
    ; Poll write-buffer ready (base+0xC)
    mov cx, 2000
.poll_cmd:
    mov dx, SB_BASE + 0xC
    in al, dx
    test al, 0x80
    jz .send_cmd
    loop .poll_cmd
.send_cmd:
    mov dx, SB_BASE + 0xC
    mov al, 0xE2
    out dx, al

    ; Poll again before parameter
    mov cx, 2000
.poll_param:
    mov dx, SB_BASE + 0xC
    in al, dx
    test al, 0x80
    jz .send_param
    loop .poll_param
.send_param:
    mov al, 0xA5
    out dx, al

    ; --- Enable interrupts; wait for IRQ (up to 0xFFFF) ---
    sti
    mov cx, 0xFFFF
.wait:
    cmp byte [irq_fired], 1
    je .check_result
    loop .wait

    ; Timeout
    cli
    mov al, 0x02
    jmp .exit

.check_result:
    cli
    ; Re-mask IRQ5
    in al, 0x21
    or al, 0x20
    out 0x21, al

    ; Read the DMA result byte
    push es
    mov ax, DMA_RESULT_SEG
    mov es, ax
    mov al, [es:0]
    pop es

    cmp al, 0xDD
    je .pass
    mov al, 0x03        ; fail: unexpected result byte
    jmp .exit

.pass:
    xor al, al
.exit:
    mov ah, 0x4C
    int 0x21

irq_handler:
    mov byte [cs:irq_fired], 1
    ; ACK 8-bit IRQ by reading DSP read-status port (base+0xE)
    mov dx, SB_BASE + 0xE
    in al, dx
    ; Non-specific EOI to master PIC
    mov al, 0x20
    out 0x20, al
    iret

irq_fired db 0
