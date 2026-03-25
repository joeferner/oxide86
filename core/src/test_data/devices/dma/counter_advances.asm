; DMA channel 0 counter-advance test
;
; Programs DMA1 channel 0 in single/verify/auto-increment mode with a known
; count, then polls current_count (port 0x01) until it changes from the
; initial value.  This mirrors what CheckIt does to verify the DMA controller
; is alive.
;
; Port map (DMA1):
;   0x0C  - clear byte flip-flop
;   0x00  - channel 0 base/current address (write low byte, then high byte)
;   0x01  - channel 0 base/current count   (write low byte, then high byte)
;   0x0B  - mode register
;   0x0A  - single-channel mask (bit 2 = 0 to unmask)
;
; Mode byte 0x54:
;   bits 7-6 = 01  single mode
;   bit  5   =  1  address auto-increment (wait... let me check)
; Actually standard 8237A mode register:
;   bits 7-6: mode (00=demand, 01=single, 10=block, 11=cascade)
;   bit  5:   address decrement (1) / increment (0)
;   bit  4:   auto-init (1=yes)
;   bits 3-2: transfer type (00=verify, 01=write, 10=read)
;   bits 1-0: channel select
;
; 0x58 = 0101_1000 = single, increment, no-auto-init, verify, ch 0

[CPU 8086]
org 0x0100

start:
    ; --- Program DMA channel 0 ---

    ; Clear flip-flop so writes start at low byte
    mov al, 0x00
    out 0x0C, al

    ; Base/current address = 0x0000
    mov al, 0x00
    out 0x00, al        ; low byte
    out 0x00, al        ; high byte

    ; Base/current count = 0x00FF (256 bytes)
    out 0x01, al        ; low byte = 0x00
    mov al, 0x00
    out 0x01, al        ; high byte = 0x00
    ; count is now 0x0000 → count register means (N+1) bytes, so 1 byte
    ; Use 0xFF for 256 bytes
    out 0x0C, al        ; clear flip-flop again
    mov al, 0xFF
    out 0x01, al        ; low byte = 0xFF
    mov al, 0x00
    out 0x01, al        ; high byte = 0x00

    ; Mode: single, increment, no-auto-init, verify, channel 0
    mov al, 0x58
    out 0x0B, al

    ; Unmask channel 0 (bit 2 = 0 = unmask, bits 1-0 = 00 = channel 0)
    mov al, 0x00
    out 0x0A, al

    ; --- Read initial count (low byte) ---
    out 0x0C, al        ; clear flip-flop (AL still 0x00, fine)
    in al, 0x01         ; read count low byte — should be 0xFF
    mov bl, al          ; BL = initial count low byte

    ; --- Poll until count changes (with timeout) ---
    mov cx, 0xFFFF      ; timeout

poll_loop:
    out 0x0C, al        ; clear flip-flop (AL still 0x00)
    in al, 0x01         ; read current count low byte
    cmp al, bl
    jne success         ; count changed → DMA is running

    loop poll_loop

    ; Timeout: DMA counters never changed
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

success:
    mov ah, 0x4C
    mov al, 0x00
    int 0x21
