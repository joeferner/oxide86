; PIT reprogramming test: verify that writing a new divisor to channel 0
; changes the interrupt rate.
;
; Strategy:
;   1. Install a custom INT 8 handler by writing directly to the IVT at
;      physical 0000:0020/0022 (bypassing DOS AH=25h).
;   2. Reprogram PIT channel 0 with divisor 0x1000 (4096).
;      At 8 MHz: cycles_per_irq = 8_000_000 * 4096 / 1_193_182 ≈ 27 464.
;   3. Enable interrupts, spin for 50 000 iterations (~900 000 cycles).
;      Expected ticks at new rate  : ~32
;      Expected ticks at default   :  ~2
;   4. Verify tick_count >= THRESHOLD — proves the new rate is in effect.
;
;   Exit 0 on success, exit 1 on failure.

[CPU 8086]
org 0x0100

THRESHOLD equ 10

start:
    cli

    ; Write custom INT 8 handler address directly into the IVT.
    ; INT 8 vector is at physical 0000:0020 (offset) and 0000:0022 (segment).
    xor ax, ax
    mov es, ax
    mov word [es:0x0020], int8_handler
    mov word [es:0x0022], cs

    ; Program PIT channel 0: control byte 0x36 = ch0, mode 3 (sq wave), lo/hi binary
    mov al, 0x36
    out 0x43, al
    mov al, 0x00            ; low byte of divisor 0x1000
    out 0x40, al
    mov al, 0x10            ; high byte of divisor 0x1000
    out 0x40, al

    ; Enable interrupts and spin for a fixed delay
    sti
    mov cx, 0xC350          ; 50 000 iterations (~900 000 cycles at ~18 cycles/iter)
.delay:
    dec cx
    jnz .delay
    cli

    ; At default rate  (~18.2 Hz):  ~2 ticks in 900 000 cycles at 8 MHz
    ; At custom rate   (~291 Hz):  ~32 ticks in 900 000 cycles at 8 MHz
    ; THRESHOLD = 10 clearly distinguishes the two cases.
    cmp word [tick_count], THRESHOLD
    jb .fail

    mov ax, 0x4C00
    int 0x21

.fail:
    mov ax, 0x4C01
    int 0x21

int8_handler:
    inc word [tick_count]
    mov al, 0x20
    out 0x20, al            ; non-specific EOI to PIC1
    iret

tick_count: dw 0
