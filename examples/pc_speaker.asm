[CPU 8086]
org 0x0100          ; .COM files start at CS:0100h

; Play "Mary Had a Little Lamb" on the PC speaker.
;
; How it works:
;   - PIT channel 2 (port 0x42) generates a square wave at the note frequency.
;   - The control word 0xB6 selects channel 2, 16-bit access, mode 3 (square wave).
;   - Port 0x61 bit 0 gates PIT channel 2; bit 1 routes the output to the speaker.
;   - Timing uses INT 1Ah (PIT tick count) at ~18.2 ticks/second.
;
; PIT input clock = 1,193,182 Hz
; Frequency divisor = 1,193,182 / desired_Hz

C4  equ 4551        ; ~262 Hz
D4  equ 4059        ; ~294 Hz
E4  equ 3615        ; ~330 Hz
G4  equ 3045        ; ~392 Hz

; Ticks per quarter note (1 tick ≈ 55 ms, so 9 ticks ≈ 500 ms ≈ 120 BPM)
BEAT equ 9

start:
    sti                 ; enable interrupts so the PIT tick counter increments

    ; Configure PIT channel 2: lobyte/hibyte, mode 3 (square wave), binary
    mov al, 0xB6
    out 0x43, al

    mov si, tune

.next:
    mov bx, [si]        ; BX = frequency divisor (0 = end of tune)
    test bx, bx
    jz  .done
    add si, 2

    mov di, [si]        ; DI = note duration in quarter-note beats
    add si, 2

    ; Load frequency divisor into PIT channel 2 (low byte then high byte)
    mov al, bl
    out 0x42, al
    mov al, bh
    out 0x42, al

    ; Enable speaker: set bit 0 (timer-2 gate) and bit 1 (speaker output)
    in  al, 0x61
    or  al, 0x03
    out 0x61, al

    ; Hold the note for DI quarter-note beats using the PIT tick counter
.beat:
    mov cx, BEAT
    mov ah, 0x00
    int 0x1a            ; DX = current tick count (low word)
    mov bx, dx          ; BX = start tick

.tick_wait:
    mov ah, 0x00
    int 0x1a            ; DX = current tick
    sub dx, bx          ; elapsed ticks since start of this beat
    cmp dx, cx          ; reached BEAT ticks?
    jb  .tick_wait

    dec di
    jnz .beat

    ; Short silence between notes
    in  al, 0x61
    and al, 0xFC        ; clear bits 0 and 1 (disable speaker gate)
    out 0x61, al

    ; Wait one tick as a gap
    mov ah, 0x00
    int 0x1a
    mov bx, dx
.gap:
    mov ah, 0x00
    int 0x1a
    cmp dx, bx
    je  .gap

    jmp .next

.done:
    ; Silence the speaker
    in  al, 0x61
    and al, 0xFC
    out 0x61, al

    mov ah, 0x4C        ; DOS terminate with return code
    mov al, 0x00        ; exit code 0
    int 0x21            ; In DOS: exits. In emulator: halts.

; -----------------------------------------------------------------------
; Tune data: pairs of (frequency_divisor, beat_count)
; Beat count is in quarter-note units (1=quarter, 2=half, 4=whole).
; A zero divisor marks the end of the tune.
; -----------------------------------------------------------------------
tune:
    ; "Mary had a little lamb"
    dw E4, 1            ; Ma-
    dw D4, 1            ; ry
    dw C4, 1            ; had
    dw D4, 1            ; a
    dw E4, 1            ; lit-
    dw E4, 1            ; tle
    dw E4, 2            ; lamb
    ; "little lamb, little lamb"
    dw D4, 1            ; lit-
    dw D4, 1            ; tle
    dw D4, 2            ; lamb
    dw E4, 1            ; lit-
    dw G4, 1            ; tle
    dw G4, 2            ; lamb
    ; "Mary had a little lamb"
    dw E4, 1            ; Ma-
    dw D4, 1            ; ry
    dw C4, 1            ; had
    dw D4, 1            ; a
    dw E4, 1            ; lit-
    dw E4, 1            ; tle
    dw E4, 1            ; lamb
    dw E4, 1            ; whose
    ; "whose fleece was white as snow"
    dw D4, 1            ; fleece
    dw D4, 1            ; was
    dw E4, 1            ; white
    dw D4, 2            ; as
    dw C4, 4            ; snow
    dw 0, 0             ; end of tune
