; INT 10h Function 06h/07h - EGA scroll clear uses fill color (not always black)
;
; Regression test for: ega_scroll_up_window and ega_scroll_down_window were
; ignoring the BH attribute and always filling cleared rows with 0 (black).
;
; Visual: top half of screen cleared to white (color 15) via AH=06h,
;         bottom half cleared to bright blue (color 9) via AH=07h.
; Then verifies VRAM plane values match the requested colors.
;
; EGA mode 0x0D planar layout (A000:0000):
;   bytes_per_row = 40, char_height = 8
;   Write mode 0, map_mask = 0x0F → writes go to all 4 planes
;   gc_read_map_select = 0 → reads return plane 0 by default
;
; Color 15 (white, 0b1111): all 4 plane bits = 1 → each byte = 0xFF
; Color 9  (bright blue, 0b1001): plane 0 bit=1 (0xFF), plane 1 bit=0 (0x00)
;
; Row offsets (pixel_y = char_row * 8):
;   row  0: offset =   0 * 40 = 0x0000
;   row 13: offset = 104 * 40 = 0x1040

[CPU 8086]
org 0x0100

start:
    ; Set EGA mode 0x0D (320x200 16-color)
    mov ah, 0x00
    mov al, 0x0D
    int 0x10

    ; Clear top half (rows 0-12) with white (color 15) via AH=06h
    mov ah, 0x06
    mov al, 0               ; lines=0 = clear window
    mov bh, 0x0F            ; fill color = white (15)
    mov ch, 0               ; top row
    mov cl, 0               ; left col
    mov dh, 12              ; bottom row
    mov dl, 39              ; right col
    int 0x10

    ; Clear bottom half (rows 13-24) with bright blue (color 9) via AH=07h
    mov ah, 0x07
    mov al, 0               ; lines=0 = clear window
    mov bh, 0x09            ; fill color = bright blue (9 = 0b1001)
    mov ch, 13              ; top row
    mov cl, 0               ; left col
    mov dh, 24              ; bottom row
    mov dl, 39              ; right col
    int 0x10

    ; Wait for key - screen is captured here for visual regression
    mov ah, 0x00
    int 0x16

    ; === VRAM verification ===
    mov ax, 0xA000
    mov es, ax

    ; --- White region (row 0, offset 0x0000) ---

    ; Plane 0 (default): bit 0 of color 15 = 1 → bytes must be 0xFF
    mov al, [es:0x0000]
    cmp al, 0xFF
    jne fail

    ; Switch to plane 1: bit 1 of color 15 = 1 → bytes must be 0xFF
    mov dx, 0x3CE
    mov al, 0x04
    out dx, al
    mov dx, 0x3CF
    mov al, 0x01
    out dx, al

    mov al, [es:0x0000]
    cmp al, 0xFF
    jne fail

    ; --- Bright blue region (row 13, offset 0x1040) ---
    ; pixel_y = 13 * 8 = 104, offset = 104 * 40 = 4160 = 0x1040

    ; Plane 1 still selected: bit 1 of color 9 = 0 → bytes must be 0x00
    mov al, [es:0x1040]
    cmp al, 0x00
    jne fail

    ; Switch to plane 0: bit 0 of color 9 = 1 → bytes must be 0xFF
    mov dx, 0x3CE
    mov al, 0x04
    out dx, al
    mov dx, 0x3CF
    mov al, 0x00
    out dx, al

    mov al, [es:0x1040]
    cmp al, 0xFF
    jne fail

    ; Restore text mode and exit
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    mov ah, 0x4C
    mov al, 0x01
    int 0x21
