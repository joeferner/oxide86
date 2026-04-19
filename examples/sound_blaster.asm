; sound_blaster.asm — Sound Blaster 16 detection and feature demo
;
; Demonstrates three features exclusive to the SB card (absent from AdLib):
;   1. DSP detection via the reset handshake (0x01/0x00 → 0xAA) and version
;      query command 0xE1 — the entire DSP subsystem is absent from AdLib
;   2. Mixer chip at ports base+4/base+5 — sets master volume to maximum
;   3. Direct DAC PCM output via DSP command 0x10 — plays a descending chirp
;      (880 → 440 → 220 → 110 Hz square wave) with software-controlled timing
;
; Also plays two FM notes through the SB's OPL ports (base+0/base+1 = 0x220/
; 0x221) rather than the AdLib-compat ports (0x388/0x389), confirming that
; both the SB port pair and the AdLib-compat pair serve the same OPL chip.
;
; NOTE: Ports > 0xFF require the DX register form of IN/OUT:
;         mov dx, 0x220 / in al, dx       (NOT: in al, 0x220)
;
; Build:  nasm -f bin sound_blaster.asm -o sound_blaster.com
; Run:    cargo run -p oxide86-native-gui -- --sound-card sb16 sound_blaster.com
;         cargo run -p oxide86-native-cli -- --sound-card sb16 sound_blaster.com

[CPU 8086]
org 0x100

SB_BASE equ 0x220

; ─── DSP detection ────────────────────────────────────────────────────────────
;
; Standard SB detection: assert the DSP reset line (base+6 ← 1), wait, then
; deassert (← 0).  The DSP responds by placing 0xAA in the read FIFO; bit 7
; of the read-status port (base+E) goes high to signal that data is waiting.
; A version query (command 0xE1) then returns two bytes: major and minor.

start:
    ; Assert reset
    mov dx, SB_BASE + 6
    mov al, 0x01
    out dx, al

    ; Short delay (~100 cycles at any clock)
    mov cx, 100
.rst_dly:
    nop
    loop .rst_dly

    ; Deassert reset
    xor al, al
    out dx, al

    ; Poll base+E bit 7 until data ready or timeout
    mov cx, 4000
.poll_aa:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .read_aa
    loop .poll_aa
    jmp .not_found          ; timeout — no DSP present

.read_aa:
    mov dx, SB_BASE + 0xA   ; DSP read data port
    in al, dx               ; should be 0xAA (ready byte)
    cmp al, 0xAA
    jne .not_found

    ; Send version query (command 0xE1, no parameters)
    call dsp_write
    db 0xE1

    call dsp_read            ; AL = major version byte
    mov [dsp_major], al
    call dsp_read            ; AL = minor version byte
    mov [dsp_minor], al

    ; Patch the version string in-place (assumes single-digit values).
    ; msg_version = "DSP version X.Y (SB16 = 4.5)"
    ;                0         1
    ;                0123456789012345
    ;                            ^  ^ offset 12 = major, offset 14 = minor
    mov al, [dsp_major]
    add al, '0'
    mov [msg_version + 12], al
    mov al, [dsp_minor]
    add al, '0'
    mov [msg_version + 14], al

    mov ah, 0x09
    mov dx, msg_found
    int 0x21
    mov dx, msg_version
    int 0x21

    jmp .detected

.not_found:
    mov ah, 0x09
    mov dx, msg_not_found
    int 0x21
    jmp .done

.detected:

; ─── MPU-401 MIDI notes via UART mode ─────────────────────────────────────────
;
; The SB16's MPU-401 controller is at ports 0x330 (data) and 0x331 (command/status).
; Reset the MPU first, then enter UART mode, then stream raw MIDI bytes.
;
; MIDI bytes for a note:
;   0x90 0x3C 0x7F  — Note On, channel 1, middle C (C4), velocity 127
;   0x80 0x3C 0x00  — Note Off, channel 1, middle C, velocity 0

    mov ah, 0x09
    mov dx, msg_mpu
    int 0x21

    ; Send MPU-401 reset command (0xFF → 0x331)
    mov dx, 0x331
    mov al, 0xFF
    out dx, al

    ; Poll status bit 7 until ACK byte is available
    mov cx, 5000
.mpu_poll_rst:
    in al, dx               ; dx = 0x331
    test al, 0x80
    jnz .mpu_read_rst
    loop .mpu_poll_rst
.mpu_read_rst:
    mov dx, 0x330
    in al, dx               ; consume 0xFE ACK

    ; Enter UART mode (0x3F → 0x331)
    mov dx, 0x331
    mov al, 0x3F
    out dx, al

    ; Poll until ACK available
    mov cx, 5000
.mpu_poll_uart:
    in al, dx               ; dx = 0x331
    test al, 0x80
    jnz .mpu_read_uart
    loop .mpu_poll_uart
.mpu_read_uart:
    mov dx, 0x330
    in al, dx               ; consume 0xFE ACK

    ; Send Note On: channel 1, middle C (0x3C), velocity 127
    mov dx, 0x330
    mov al, 0x90            ; Note On, channel 1
    out dx, al
    mov al, 0x3C            ; Middle C
    out dx, al
    mov al, 0x7F            ; velocity 127
    out dx, al

    call delay_long

    ; Send Note Off: channel 1, middle C, velocity 0
    mov al, 0x80            ; Note Off, channel 1
    out dx, al
    mov al, 0x3C
    out dx, al
    xor al, al
    out dx, al

; ─── Mixer — set master volume to maximum ─────────────────────────────────────
;
; The mixer is accessed via an index/data pair at base+4 (index write) and
; base+5 (data read/write).  AdLib has no mixer at all.
;
; Write both SBPro-style register 0x22 (4-bit L nibble, 4-bit R nibble) and
; SB16-style registers 0x30/0x31 (5-bit per channel, MSBs, 0xF8 = max).

    ; SBPro master volume (reg 0x22): 0xFF = both channels full
    mov dx, SB_BASE + 4
    mov al, 0x22
    out dx, al
    mov dx, SB_BASE + 5
    mov al, 0xFF
    out dx, al

    ; SB16 master volume left (reg 0x30): upper 5 bits, 0xF8 = maximum
    mov dx, SB_BASE + 4
    mov al, 0x30
    out dx, al
    mov dx, SB_BASE + 5
    mov al, 0xF8
    out dx, al

    ; SB16 master volume right (reg 0x31)
    mov dx, SB_BASE + 4
    mov al, 0x31
    out dx, al
    mov dx, SB_BASE + 5
    mov al, 0xF8
    out dx, al

    mov ah, 0x09
    mov dx, msg_mixer
    int 0x21

; ─── Speaker on (DSP command 0xD1) ───────────────────────────────────────────
;
; Enables DAC output.  Some SB models are silent during DMA without this.
; The AdLib card has no speaker control concept.

    call dsp_write
    db 0xD1

; ─── Direct DAC PCM — descending chirp ───────────────────────────────────────
;
; DSP command 0x10 (Direct DAC, 8-bit) sends one sample to the DAC immediately.
; No DMA or IRQ is required.  The sample rate is controlled entirely by the
; delay between consecutive 0x10 writes — the defining difference from AdLib.
;
; Waveform: square wave alternating 0xC0 (+64 above midpoint) and 0x40 (−64).
; Four tones descend one octave at a time: ~880 → 440 → 220 → 110 Hz.
;
; Timing (8 MHz): NOP = 3 cycles, LOOP = 5 cycles → ~8 cycles/iteration.
;   half_period = 568 → 568 × 8 = 4544 cycles → T/2 ≈ 568 µs → ~880 Hz.
; Each tone plays for 60 half-cycles (30 full cycles, ≈ 34 ms per tone).

    mov ah, 0x09
    mov dx, msg_pcm
    int 0x21

    mov word [half_period], 568
    mov byte [tone_count], 4

.tone_loop:
    mov byte [half_cycles], 60

.half_hi:
    ; Output high sample (0xC0 = loud positive in unsigned 8-bit PCM)
    mov al, 0xC0
    call play_sample

    mov cx, [half_period]
.dly_hi:
    nop
    loop .dly_hi

.half_lo:
    ; Output low sample (0x40 = loud negative)
    mov al, 0x40
    call play_sample

    mov cx, [half_period]
.dly_lo:
    nop
    loop .dly_lo

    dec byte [half_cycles]
    jnz .half_hi

    shl word [half_period], 1      ; double the period → halve the frequency
    dec byte [tone_count]
    jnz .tone_loop

    ; Send one silence sample to leave the DAC at the midpoint
    mov al, 0x80
    call play_sample

; ─── Speaker off (DSP command 0xD3) ──────────────────────────────────────────

    call dsp_write
    db 0xD3

; ─── OPL FM via the SB's own ports (base+0 / base+1 = 0x220 / 0x221) ─────────
;
; The SB16 exposes its OPL3 chip at both 0x220/0x221 (SB port pair) and
; 0x388/0x389 (AdLib-compat pair).  This section plays the same two notes as
; examples/adlib.asm but accesses the chip through 0x220/0x221, proving that
; the SB port pair is live in addition to the AdLib-compat pair.

    mov ah, 0x09
    mov dx, msg_opl
    int 0x21

    ; Enable OPL waveform select (reg 0x01 bit 5)
    call sb_opl_write
    db 0x01, 0x20

    ; ── Operator slot 0 (modulator, channel 0) ──
    ; reg 0x20: EG=1 MULT=1 (sustained, multiply ×1)
    call sb_opl_write
    db 0x20, 0x21
    ; reg 0x40: TL=16 (moderate volume; lower value = louder, range 0–63)
    call sb_opl_write
    db 0x40, 0x10
    ; reg 0x60: AR=15 DR=0 (instant attack, no decay)
    call sb_opl_write
    db 0x60, 0xF0
    ; reg 0x80: SL=0 RR=7 (sustain level 0, release rate 7)
    call sb_opl_write
    db 0x80, 0x07
    ; reg 0xE0: waveform = sine
    call sb_opl_write
    db 0xE0, 0x00

    ; ── Operator slot 3 (carrier, channel 0) ──
    ; reg 0x23: EG=1 MULT=1
    call sb_opl_write
    db 0x23, 0x21
    ; reg 0x43: TL=0 (full volume for carrier)
    call sb_opl_write
    db 0x43, 0x00
    ; reg 0x63: AR=15 DR=0
    call sb_opl_write
    db 0x63, 0xF0
    ; reg 0x83: SL=0 RR=7
    call sb_opl_write
    db 0x83, 0x07
    ; reg 0xE3: waveform = sine
    call sb_opl_write
    db 0xE3, 0x00

    ; Channel 0 feedback/algorithm: FM (algo=0), feedback=4
    call sb_opl_write
    db 0xC0, 0x08

    ; ── Note 1: A4 (440 Hz) — fnum=0x244, block=4, key_on ──
    call sb_opl_write
    db 0xA0, 0x44           ; fnum low byte
    call sb_opl_write
    db 0xB0, 0x32           ; key_on=1, block=4, fnum_hi=2

    call delay_long

    ; ── Note 2: D5 (~587 Hz) — fnum=0x308, block=4, key_on ──
    call sb_opl_write
    db 0xA0, 0x08           ; fnum low byte
    call sb_opl_write
    db 0xB0, 0x33           ; key_on=1, block=4, fnum_hi=3

    call delay_long

    ; Key off: clear key_on bit in channel 0
    call sb_opl_write
    db 0xB0, 0x13           ; key_on=0, block=4, fnum_hi=3

.done:
    mov ah, 0x4C
    xor al, al
    int 0x21

; ─── Subroutine: dsp_write ────────────────────────────────────────────────────
; Reads one inline byte after the CALL instruction and writes it to the DSP
; command/data port (base+C), polling the write-buffer-busy bit (bit 7) first.
;
; The byte is saved on the stack with PUSH AX before the poll loop so that
; the IN instruction cannot clobber it.
;
; Corrupts: AX, BX, CX, DX.
dsp_write:
    pop bx                  ; BX = address of the inline data byte
    mov al, [bx]            ; AL = byte to send
    inc bx                  ; advance past the inline byte → updated return addr
    push bx                 ; save updated return address on stack
    push ax                 ; save byte across the poll loop (IN clobbers AL)

    ; Poll base+C bit 7 until write buffer is free (bit 7 = 0)
    mov cx, 4000
.dw_poll:
    mov dx, SB_BASE + 0xC
    in al, dx
    test al, 0x80
    jz .dw_send
    loop .dw_poll

.dw_send:
    pop ax                  ; AL = byte to send (DX still = SB_BASE + 0xC)
    out dx, al
    ret                     ; pops updated return addr → resumes after the db byte

; ─── Subroutine: dsp_read ─────────────────────────────────────────────────────
; Polls base+E bit 7 until the DSP read FIFO is non-empty, then reads and
; returns the next byte in AL.
;
; Corrupts: AX, CX, DX.
dsp_read:
    mov cx, 4000
.dr_poll:
    mov dx, SB_BASE + 0xE   ; read-buffer status port
    in al, dx
    test al, 0x80           ; bit 7 = data available
    jnz .dr_read
    loop .dr_poll
.dr_read:
    mov dx, SB_BASE + 0xA   ; DSP read data port
    in al, dx
    ret

; ─── Subroutine: play_sample ──────────────────────────────────────────────────
; Sends DSP command 0x10 (Direct DAC, 8-bit) followed by the sample byte in AL.
; Used instead of the inline-byte dsp_write convention in the PCM loop because
; it avoids clobbering the caller's BX register (needed for the half_period
; memory reads after each call).
;
; Corrupts: AX, CX, DX.  Preserves: BX, SI, DI, BP.
play_sample:
    push ax                 ; save sample byte across the command write

    ; Poll until write buffer ready, then send Direct DAC command
    mov cx, 400
.ps_poll_cmd:
    mov dx, SB_BASE + 0xC
    in al, dx
    test al, 0x80
    jz .ps_cmd
    loop .ps_poll_cmd
.ps_cmd:
    mov al, 0x10            ; Direct DAC command
    out dx, al              ; DX still = SB_BASE + 0xC

    ; Poll until write buffer ready, then send the sample byte
    mov cx, 400
.ps_poll_data:
    in al, dx               ; DX still = SB_BASE + 0xC
    test al, 0x80
    jz .ps_data
    loop .ps_poll_data
.ps_data:
    pop ax                  ; AL = sample byte
    out dx, al
    ret

; ─── Subroutine: sb_opl_write ─────────────────────────────────────────────────
; Reads two inline bytes [reg_index, reg_value] after the CALL and writes them
; to the SB card's OPL address/data port pair (base+0 / base+1 = 0x220/0x221),
; with the register-address and post-data delays required by the OPL chip.
;
; Identical in structure to adlib_write_reg in examples/adlib.asm, but targets
; the SB port pair instead of 0x388/0x389.
;
; Corrupts: AX, BX, CX, DX.
sb_opl_write:
    pop bx                  ; BX = address of inline [reg, val] pair
    mov al, [bx]            ; AL = register index
    mov dx, SB_BASE + 0     ; OPL address port (0x220)
    out dx, al
    ; Address-setup delay: OPL requires ≥3.3 µs between address and data writes.
    ; 8 NOPs ≈ 24 cycles ≈ 3 µs at 8 MHz — adequate for the emulator.
    mov cx, 8
.opl_addr_dly:
    nop
    loop .opl_addr_dly
    mov al, [bx + 1]        ; AL = register value
    mov dx, SB_BASE + 1     ; OPL data port (0x221)
    out dx, al
    ; Post-data delay: OPL requires ≥23 µs after a data write.
    ; 48 NOPs ≈ 144 cycles ≈ 18 µs — sufficient for the emulator.
    mov cx, 48
.opl_data_dly:
    nop
    loop .opl_data_dly
    add bx, 2               ; skip past the two inline bytes
    push bx                 ; push updated return address
    ret

; ─── Subroutine: delay_long ───────────────────────────────────────────────────
; Busy-wait approximately 0.5 seconds at 4.77 MHz.
; At higher clock speeds the delay is proportionally shorter; adjust the outer
; loop count (DX) if a different duration is needed.
delay_long:
    push cx
    push dx
    mov dx, 30              ; outer iterations (30 × 65535 NOPs ≈ 0.43 s at 4.77 MHz)
.dl_outer:
    mov cx, 0xFFFF
.dl_inner:
    nop
    loop .dl_inner
    dec dx
    jnz .dl_outer
    pop dx
    pop cx
    ret

; ─── Data ─────────────────────────────────────────────────────────────────────
dsp_major    db 0
dsp_minor    db 0
half_period  dw 0
tone_count   db 0
half_cycles  db 0

msg_found     db 'Sound Blaster detected.', 0x0D, 0x0A, '$'
msg_mpu       db 'MPU-401: sending MIDI note via UART mode...', 0x0D, 0x0A, '$'
msg_not_found db 'Sound Blaster not found (no DSP ready byte).', 0x0D, 0x0A, '$'
msg_version   db 'DSP version X.Y (SB16 = 4.5)', 0x0D, 0x0A, '$'
;                            ^^ patched at runtime — offset 12 = major, 14 = minor
msg_mixer     db 'Mixer: master volume set to maximum.', 0x0D, 0x0A, '$'
msg_pcm       db 'PCM: playing descending chirp via Direct DAC (880-440-220-110 Hz)...', 0x0D, 0x0A, '$'
msg_opl       db 'OPL: playing two FM notes via SB port 0x220/0x221...', 0x0D, 0x0A, '$'
