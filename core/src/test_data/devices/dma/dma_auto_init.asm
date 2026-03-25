; DMA auto-init test (channel 1)
;
; Verifies that a channel programmed with auto-init reloads its base
; registers on Terminal Count and remains active (unmasked), rather than
; being masked like a non-auto-init channel would be.
;
; Strategy:
;   1. Program channel 1 with count=0 (TC after every DMA cycle) and
;      auto-init enabled, verify transfer type (no memory writes needed).
;   2. Software-assert DREQ and unmask the channel.
;   3. Spin long enough for many TC events to occur.
;   4. Read the all-channel mask register (port 0x0F).
;      - Auto-init working: channel 1 bit is CLEAR (still running).
;      - Auto-init broken:  channel 1 bit is SET (masked on TC).
;
; DMA1 channel 1 port map:
;   0x02  base/current address (two bytes via flip-flop)
;   0x03  base/current count   (two bytes via flip-flop)
;   0x09  single-channel request (software DREQ)
;   0x0A  single-channel mask
;   0x0B  mode register
;   0x0C  clear byte-pointer flip-flop
;   0x0F  all-channel mask register (read)
;   0x83  page register for channel 1
;
; Mode byte 0x51 (verify, single, auto-init, channel 1):
;   bits 7-6: 01 = single mode
;   bit  5:   0  = address increment
;   bit  4:   1  = auto-init
;   bits 3-2: 00 = verify (no transfer)
;   bits 1-0: 01 = channel 1
;
; Exit codes:
;   0x00  pass — channel 1 is still unmasked after many TC events
;   0x01  fail — channel 1 was masked (auto-init did not reload)

[CPU 8086]
org 0x0100

start:
    ; --- Mask channel 1 before programming ---
    mov  al, 0x05          ; ch=1, set-mask
    out  0x0A, al

    ; --- Clear flip-flop ---
    xor  al, al
    out  0x0C, al

    ; Channel 1 address = 0x0000 (irrelevant for verify mode)
    out  0x02, al
    out  0x02, al

    ; Page = 0 (irrelevant for verify mode)
    out  0x83, al

    ; Clear flip-flop before count
    out  0x0C, al

    ; Count = 0 → TC fires after 1 DMA cycle, then reloads and repeats
    out  0x03, al          ; low byte = 0
    out  0x03, al          ; high byte = 0

    ; Mode: verify, single, auto-init, ch1
    mov  al, 0x51
    out  0x0B, al

    ; Software-assert DREQ on channel 1
    mov  al, 0x05          ; ch=1, set-request
    out  0x09, al

    ; Unmask channel 1
    mov  al, 0x01          ; ch=1, clear-mask
    out  0x0A, al

    ; --- Spin to let many TC events occur ---
    ; count=0 → TC every 1 DMA cycle (every 4 CPU cycles).
    ; 0x400 iterations × ~5 cycles/iter = ~5120 CPU cycles = ~1280 TC events.
    mov  cx, 0x0400
.spin:
    loop .spin

    ; --- Check all-channel mask register (port 0x0F) ---
    in   al, 0x0F
    test al, 0x02          ; bit 1 = channel 1
    jnz  fail              ; set → channel 1 was masked → auto-init broken

    ; Pass
    mov  ah, 0x4C
    xor  al, al
    int  0x21

fail:
    mov  ah, 0x4C
    mov  al, 0x01
    int  0x21
