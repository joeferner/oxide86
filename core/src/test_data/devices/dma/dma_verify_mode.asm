; DMA verify-mode test (channel 1)
;
; Verifies that a channel programmed in verify mode (transfer type 00)
; does NOT write anything to memory even though the DMA runs.
;
; Strategy:
;   1. Pre-fill a buffer with sentinel value 0xCC.
;   2. Program DMA channel 1 in verify mode with a large count so that
;      TC does not fire during the test window.
;   3. Software-assert DREQ and unmask channel 1 to let the DMA run.
;   4. Spin long enough for many DMA cycles to elapse.
;   5. Verify every byte of the buffer is still 0xCC.
;
; DMA1 channel 1 port map:
;   0x02  base/current address (two bytes via flip-flop)
;   0x03  base/current count   (two bytes via flip-flop)
;   0x09  single-channel request (software DREQ)
;   0x0A  single-channel mask
;   0x0B  mode register
;   0x0C  clear byte-pointer flip-flop
;   0x83  page register for channel 1
;
; Mode byte 0x41 (verify, single, no-auto-init, channel 1):
;   bits 7-6: 01 = single mode
;   bit  5:   0  = address increment
;   bit  4:   0  = no auto-init
;   bits 3-2: 00 = verify (no transfer)
;   bits 1-0: 01 = channel 1
;
; Exit codes:
;   0x00  pass — buffer untouched
;   0x01  fail — buffer was modified (verify mode wrote to memory)

[CPU 8086]
org 0x0100

BUF_SIZE equ 16

start:
    ; --- Pre-fill buffer with sentinel 0xCC ---
    cld
    mov  di, buf
    mov  cx, BUF_SIZE
    mov  al, 0xCC
    rep  stosb

    ; --- Program DMA channel 1 ---

    ; Mask channel 1 before programming
    mov  al, 0x05          ; ch=1, set-mask
    out  0x0A, al

    ; Clear flip-flop
    xor  al, al
    out  0x0C, al

    ; Channel 1 address = physical address of buf.
    ; Physical = DS * 16 + offset(buf).  Low 16 bits = (DS << 4) + offset.
    ; Use CL-based shift (valid on 8086).
    mov  ax, ds
    mov  cl, 4
    shl  ax, cl            ; AX = (DS << 4) & 0xFFFF
    add  ax, buf
    out  0x02, al          ; low byte
    xchg al, ah
    out  0x02, al          ; high byte

    ; Page register: bits 19:16 = DS >> 12
    mov  ax, ds
    mov  cl, 12
    shr  ax, cl
    out  0x83, al

    ; Clear flip-flop before count
    out  0x0C, al

    ; Count = 0xFFFF — TC will not fire during our short spin window
    mov  ax, 0xFFFF
    out  0x03, al          ; low byte = 0xFF
    xchg al, ah
    out  0x03, al          ; high byte = 0xFF

    ; Mode: verify, single, no-auto-init, ch1
    mov  al, 0x41
    out  0x0B, al

    ; Software-assert DREQ on channel 1
    mov  al, 0x05          ; ch=1, set-request
    out  0x09, al

    ; Unmask channel 1
    mov  al, 0x01          ; ch=1, clear-mask
    out  0x0A, al

    ; --- Spin to let DMA run ---
    ; 0x400 iters × ~5 cycles/iter = ~5120 CPU cycles = ~1280 DMA cycles.
    ; count=0xFFFF → TC needs 65536 DMA cycles, so TC will not fire here.
    mov  cx, 0x0400
.spin:
    loop .spin

    ; --- Mask channel 1 and release software DREQ ---
    mov  al, 0x05
    out  0x0A, al
    mov  al, 0x01
    out  0x09, al

    ; --- Verify buffer is still 0xCC throughout ---
    mov  si, buf
    mov  cx, BUF_SIZE
.check:
    lodsb
    cmp  al, 0xCC
    jne  fail
    loop .check

    ; Pass
    mov  ah, 0x4C
    xor  al, al
    int  0x21

fail:
    mov  ah, 0x4C
    mov  al, 0x01
    int  0x21

buf:
    times BUF_SIZE db 0x00
