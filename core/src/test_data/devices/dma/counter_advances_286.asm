; DMA2 channel 5 counter-advance test (286 system)
;
; Programs DMA2 channel 5 (DMA2 internal ch 1) in single/verify mode,
; software-asserts DREQ via the request register, then polls its
; current-count register until the low byte changes from the initial
; value.  Mirrors what CheckIt does for DMA2 on a 286 AT.
;
; DMA2 channel 4 (DMA2 internal ch 0) is the cascade channel that
; connects DMA1's HRQ.  Channels 5-7 are available for transfers.
;
; Port map (DMA2, word-spaced: phys = 0xC0 + offset*2):
;   0xDA  master clear        (offset 0x0D)
;   0xD8  clear flip-flop     (offset 0x0C)
;   0xD6  mode register       (offset 0x0B)
;   0xD4  single channel mask (offset 0x0A)
;   0xD2  request register    (offset 0x09)  software DREQ
;   0xC4  ch5 address         (offset 0x02)
;   0xC6  ch5 count           (offset 0x03)
;
; Mode byte 0x41 for ch5:
;   bits 7-6: 01 = single mode
;   bit  5:   0  = address increment
;   bit  4:   0  = no auto-init
;   bits 3-2: 00 = verify (no data movement)
;   bits 1-0: 01 = DMA2 internal channel 1 (global channel 5)

[CPU 286]
org 0x0100

start:
    ; DMA2 master clear
    mov al, 0x00
    out 0xDA, al

    ; Clear flip-flop
    out 0xD8, al

    ; ch5 base/current address = 0x0000
    out 0xC4, al        ; low byte
    out 0xC4, al        ; high byte

    ; ch5 base/current count = 0x00FF  (256 bytes)
    out 0xD8, al        ; clear flip-flop (AL still 0x00)
    mov al, 0xFF
    out 0xC6, al        ; low byte = 0xFF
    mov al, 0x00
    out 0xC6, al        ; high byte = 0x00

    ; Mode: single, increment, no-auto-init, verify, DMA2 internal ch1
    mov al, 0x41
    out 0xD6, al

    ; Unmask ch5 (bit 2 = 0 unmask, bits 1-0 = 01 = DMA2 ch1)
    mov al, 0x01
    out 0xD4, al

    ; Software-assert DREQ on ch5 (bit 2 = 1 set, bits 1-0 = 01)
    mov al, 0x05
    out 0xD2, al

    ; Read initial count low byte
    mov al, 0x00
    out 0xD8, al        ; clear flip-flop
    in  al, 0xC6        ; ch5 count low byte — should be 0xFF
    mov bl, al

    ; Poll until count changes (with timeout)
    mov cx, 0xFFFF

poll_loop:
    mov al, 0x00
    out 0xD8, al        ; clear flip-flop
    in  al, 0xC6        ; read current count low byte
    cmp al, bl
    jne success         ; count changed → DMA2 is running

    loop poll_loop

    ; Timeout: DMA2 counters never changed
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

success:
    mov ah, 0x4C
    mov al, 0x00
    int 0x21
