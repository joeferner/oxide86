; CGA Snow-Avoidance Write Test
;
; Tests that port 0x3DA bit 0 (horizontal retrace) toggles correctly.
; Uses the classic CGA snow-avoidance write pattern:
;   1. Wait for bit 0 CLEAR (display active — end of any prior retrace)
;   2. CLI
;   3. Wait for bit 0 SET  (horizontal retrace begins)
;   4. STOSW into B800     (write during safe retrace window)
;   5. STI
;
; Without bit 0 toggling, step 1 or 3 spins forever and nothing is written.
; With it working, "HELLO" appears at the top-left of the 80x25 text screen.

[CPU 8086]
org 0x100

start:
    ; Enter 80x25 color text mode
    mov ax, 0x0003
    int 0x10

    ; Point ES at CGA text buffer
    mov ax, 0xB800
    mov es, ax

    ; Write "HELLO" using snow-avoidance pattern
    ; Each word: low byte = ASCII char, high byte = attribute (0x07 = grey on black)
    mov di, 0           ; top-left cell (row 0, col 0)
    mov dx, 0x03DA

    mov bx, ('H' | (0x07 << 8))
    call snow_write

    mov bx, ('E' | (0x07 << 8))
    call snow_write

    mov bx, ('L' | (0x07 << 8))
    call snow_write

    mov bx, ('L' | (0x07 << 8))
    call snow_write

    mov bx, ('O' | (0x07 << 8))
    call snow_write

    ; Wait for keypress then exit
    mov ah, 0x00
    int 0x16

    mov ax, 0x4C00
    int 0x21

; snow_write — write BX to ES:DI (advances DI by 2)
; Uses the classic double-poll pattern:
;   wait for display active (bit 0 = 0), then
;   CLI, wait for retrace start (bit 0 = 1), STOSW, STI.
snow_write:
    ; Step 1: wait for bit 0 to be CLEAR (display active)
    mov ah, 0x01
.wait_display:
    in al, dx
    test ah, al
    jne .wait_display

    ; Step 2: disable interrupts, then wait for retrace START (bit 0 SET)
    cli
.wait_retrace:
    in al, dx
    test ah, al
    je .wait_retrace

    ; Step 3: write the word during the retrace window
    mov ax, bx
    stosw

    sti
    ret
