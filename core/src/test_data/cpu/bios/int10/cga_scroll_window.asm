; INT 10h - Scroll in CGA graphics mode 4 (320x200 4-colour)
; Tests cga_scroll_window via AH=06h (scroll up) and AH=07h (scroll down).
;
; CGA mode 4 memory layout (interleaved banks):
;   Even pixel rows: B8000 + (pixel_y / 2) * 80
;   Odd  pixel rows: B8000 + 0x2000 + (pixel_y / 2) * 80
;   Each char cell = 8x8 pixels; each char column = 2 bytes (2 bpp)
;
; Part 1 – scroll up (AH=06h):
;   Write 0xAA at char row 1, col 0, pixel row 0  (B800:0140)
;     pixel_y = 8, bank = 0 (even), row_start = (8/2)*80 = 0x140
;   Scroll up 1 line across full screen (rows 0-24, cols 0-39)
;   Verify B800:0000 == 0xAA  (moved to char row 0)
;   Verify B800:0140 == 0x00  (char row 1 cleared to black)
;
; Part 2 – scroll down (AH=07h, re-enter mode 4 to clear framebuffer):
;   Write 0xBB at char row 0, col 0, pixel row 0  (B800:0000)
;   Scroll down 1 line across full screen
;   Verify B800:0140 == 0xBB  (moved to char row 1)
;   Verify B800:0000 == 0x00  (char row 0 cleared to black)

[CPU 8086]
org 0x0100

start:
    ;------------------------------------------------------------------
    ; Part 1: scroll up
    ;------------------------------------------------------------------
    mov ah, 0x00
    mov al, 0x04            ; CGA mode 4: 320x200 4-colour
    int 0x10

    ; Write sentinel 0xAA to char row 1, col 0, pixel row 0
    mov ax, 0xB800
    mov es, ax
    mov byte [es:0x0140], 0xAA

    ; Scroll up 1 line across the whole screen
    mov ah, 0x06
    mov al, 1               ; lines to scroll
    mov bh, 0x00            ; blank-line fill colour (black)
    mov ch, 0               ; top row
    mov cl, 0               ; left col
    mov dh, 24              ; bottom row
    mov dl, 39              ; right col (40 chars wide)
    int 0x10

    ; char row 0, col 0, pixel row 0 → B800:0000 must be 0xAA
    mov al, [es:0x0000]
    cmp al, 0xAA
    jne fail

    ; char row 1 (now blank) → B800:0140 must be 0x00
    mov al, [es:0x0140]
    cmp al, 0x00
    jne fail

    ;------------------------------------------------------------------
    ; Part 2: scroll down (re-enter mode 4 to clear framebuffer)
    ;------------------------------------------------------------------
    mov ah, 0x00
    mov al, 0x04
    int 0x10

    mov ax, 0xB800
    mov es, ax

    ; Write sentinel 0xBB to char row 0, col 0, pixel row 0  (B800:0000)
    mov byte [es:0x0000], 0xBB

    ; Scroll down 1 line across the whole screen
    mov ah, 0x07
    mov al, 1
    mov bh, 0x00
    mov ch, 0
    mov cl, 0
    mov dh, 24
    mov dl, 39
    int 0x10

    ; char row 1, col 0, pixel row 0 → B800:0140 must be 0xBB
    mov al, [es:0x0140]
    cmp al, 0xBB
    jne fail

    ; char row 0 (now blank) → B800:0000 must be 0x00
    mov al, [es:0x0000]
    cmp al, 0x00
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
