; INT 10h Function 07h - Scroll Down in EGA mode 0x0D (320x200 16-colour)
; Tests ega_scroll_down_window.
;
; EGA mode 0x0D planar memory layout (at A000:0000):
;   bytes_per_row = 40 (320 pixels / 8 pixels per byte)
;   char_height   = 8 pixels
;   offset = pixel_y * 40 + char_col   where pixel_y = char_row * 8 + pixel_row
;
; EGA default register state after mode set:
;   Write mode 0, sequencer_map_mask = 0x0F (all 4 planes)
;   gc_read_map_select = 0 (read from plane 0)
;   gc_bit_mask = 0xFF, gc_function_select = 0 (direct)
;   → writing val writes val to all planes; reading returns plane-0 byte
;
; Test:
;   Write 0xBB to char row 0, col 0, pixel row 0  (A000:0000)
;   Scroll down 1 line across full screen (rows 0-24, cols 0-39)
;   Verify A000:0140 == 0xBB  (moved to char row 1)
;     pixel_y = 1*8+0 = 8  →  offset = 8*40 + 0 = 320 = 0x140
;   Verify A000:0000 == 0x00  (char row 0 cleared to black)

[CPU 8086]
org 0x0100

start:
    ; Set EGA graphics mode 0x0D (320x200 16-colour)
    mov ah, 0x00
    mov al, 0x0D
    int 0x10

    ; Write sentinel 0xBB to char row 0, col 0, pixel row 0
    ; pixel_y = 0  →  offset = 0*40 + 0 = 0
    mov ax, 0xA000
    mov es, ax
    mov byte [es:0x0000], 0xBB

    ; Scroll down 1 line across the whole screen
    mov ah, 0x07
    mov al, 1               ; lines to scroll
    mov bh, 0x00            ; blank-line fill (black)
    mov ch, 0               ; top row
    mov cl, 0               ; left col
    mov dh, 24              ; bottom row
    mov dl, 39              ; right col (40 chars wide)
    int 0x10

    ; char row 1, col 0, pixel row 0 → A000:0140 must be 0xBB
    mov al, [es:0x0140]
    cmp al, 0xBB
    jne fail

    ; char row 0 (now blank) → A000:0000 must be 0x00
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
