; EGA two-pass sprite rendering — noise vs correct transparency test
; Mode 10h (640x350, 16-color planar, 80 bytes/row)
;
; The two-pass algorithm used by Lemmings and similar EGA games:
;
;   Pass 1 (shape mask):
;     Sequencer Map Mask = 0x0F (all planes)
;     GC Enable Set/Reset = 0x0E  (planes 1-3 forced to Set/Reset=0; plane 0 = CPU data)
;     GC Bit Mask = glyph byte
;     For each byte: dummy read to latch background, then write glyph byte.
;     Result: sprite pixels have planes 1-3 zeroed; background pixels unchanged.
;
;   Pass 2 (color fill):
;     GC Mode = 0x08 (Read Mode 1 | Write Mode 0)
;     GC Data Rotate = 0x10 (OR ALU)
;     GC Color Compare = 0x08  (match plane 3 = 1)
;     GC Color Don't Care = 0x08  (only compare plane 3)
;     Sequencer Map Mask = 0x06 (write planes 1, 2)
;     For each byte: Read Mode 1 returns bitmask of pixels where plane 3=1
;       (background). NOT that = bitmask of sprite pixels (plane 3=0 from pass 1).
;       Set GC Bit Mask = inverted result. Write 0xFF (OR fills planes 1,2 in mask).
;
; NOISE BUG:
;   After a fresh black screen clear, ALL pixels have plane 3=0.
;   Read Mode 1 returns 0x00 for every byte (no pixel matches plane 3=1).
;   NOT 0x00 = 0xFF — every pixel in the bounding box appears to be a sprite pixel.
;   Pass 2 fills the entire bounding box with solid sprite color instead of the
;   glyph outline, because the transparency key (plane 3=1 for background) is absent.
;
; Step 1: Demonstrate noise — run both passes on a black screen.
;   Expected: solid filled rectangle (both border AND interior filled with color).
;
; Step 2: Demonstrate correct — draw bright background (plane 3=1 everywhere),
;   then run both passes. Background transparency key is now valid.
;   Expected: hollow rectangle outline in sprite color; interior shows background.

[CPU 8086]
org 0x100

SCREEN_SEG    equ 0xA000
BYTES_PER_ROW equ 80
SPRITE_W      equ 8        ; sprite width in bytes (64 pixels)
SPRITE_H      equ 16       ; sprite height in scanlines
SPRITE_ROW    equ 167      ; top scanline of sprite (≈ vertical center)
SPRITE_COL    equ 36       ; byte offset within row (≈ horizontal center)

; Precomputed: SPRITE_ROW * BYTES_PER_ROW + SPRITE_COL = 167*80+36 = 13396
SPRITE_OFFSET equ SPRITE_ROW * BYTES_PER_ROW + SPRITE_COL

start:
    ; Switch to EGA Mode 10h (640x350, 16 colors). BIOS clears VRAM to 0.
    mov ah, 0x00
    mov al, 0x10
    int 0x10

    mov ax, SCREEN_SEG
    mov es, ax

    ; ── Step 1: Noise case ─────────────────────────────────────────────────
    ; VRAM is all zeros after mode set: plane 3 = 0 everywhere.
    ; Pass 2 sees no background key and fills the entire bounding box.

    call do_pass1
    call do_pass2

    ; Halt for screenshot (step 1 — noise: solid filled rectangle visible)
    mov ah, 0x00
    int 0x16

    ; ── Step 2: Correct case ───────────────────────────────────────────────
    ; Reset mode (BIOS clears VRAM to 0 again).
    mov ah, 0x00
    mov al, 0x10
    int 0x10

    mov ax, SCREEN_SEG
    mov es, ax

    ; Fill plane 3 for entire screen with 0xFF (all pixels: plane 3 = 1).
    ; This sets the background transparency key. Color = 8 (0b1000 = dark gray).
    mov dx, 0x3C4
    mov al, 0x02            ; Sequencer Map Mask register index
    out dx, al
    inc dx
    mov al, 0x08            ; write plane 3 only
    out dx, al

    xor di, di
    mov cx, BYTES_PER_ROW * 350
    mov al, 0xFF
    rep stosb               ; plane 3 = 1 for every pixel on screen

    ; Restore Map Mask to all planes before the two-pass render.
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    inc dx
    mov al, 0x0F
    out dx, al

    ; Now run the same two-pass render. Pass 1 carves the glyph shape by
    ; zeroing plane 3 in sprite pixels. Pass 2 uses Read Mode 1 to find
    ; those pixels (plane 3=0) and fills them with sprite color only.
    call do_pass1
    call do_pass2

    ; Halt for screenshot (step 2 — correct: hollow outline on dark background)
    mov ah, 0x00
    int 0x16

    ; Exit
    mov ah, 0x4C
    mov al, 0x00
    int 0x21


; ── Pass 1: shape mask ────────────────────────────────────────────────────────
; For each byte of the sprite glyph at VRAM offset (SPRITE_ROW, SPRITE_COL):
;   Set GC Bit Mask = glyph byte, latch background via dummy read, write glyph.
;   Enable Set/Reset=0x0E forces planes 1-3 to 0 at masked bits; plane 0 = glyph.
; After this pass, sprite pixels have planes 1-3 = 0.
do_pass1:
    push ax
    push bx
    push cx
    push dx
    push si
    push di

    ; Sequencer Map Mask = 0x0F (write all 4 planes)
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    inc dx
    mov al, 0x0F
    out dx, al

    ; GC Set/Reset = 0x00 (set/reset value for all planes = 0)
    mov dx, 0x3CE
    mov al, 0x00
    out dx, al
    inc dx
    mov al, 0x00
    out dx, al

    ; GC Enable Set/Reset = 0x0E (planes 1,2,3 use set/reset; plane 0 = CPU data)
    mov dx, 0x3CE
    mov al, 0x01
    out dx, al
    inc dx
    mov al, 0x0E
    out dx, al

    ; GC Mode = 0x00 (Read Mode 0, Write Mode 0, no ALU)
    mov dx, 0x3CE
    mov al, 0x05
    out dx, al
    inc dx
    mov al, 0x00
    out dx, al

    mov di, SPRITE_OFFSET
    mov si, sprite_glyph
    mov bx, SPRITE_H

.row_loop:
    mov cx, SPRITE_W

.byte_loop:
    mov ah, [si]            ; ah = current glyph byte

    ; GC Bit Mask = glyph byte (gates which pixels are affected)
    mov dx, 0x3CE
    mov al, 0x08
    out dx, al
    inc dx
    mov al, ah
    out dx, al

    mov al, [es:di]         ; dummy read: load hardware latches with background
    mov al, ah
    mov [es:di], al         ; write: plane 0 = glyph bits, planes 1-3 = 0 (set/reset)

    inc si
    inc di
    loop .byte_loop

    add di, BYTES_PER_ROW - SPRITE_W   ; advance to same column on next row
    dec bx
    jnz .row_loop

    ; Restore GC registers to defaults
    mov dx, 0x3CE
    mov al, 0x01            ; Enable Set/Reset = 0x00 (disable)
    out dx, al
    inc dx
    mov al, 0x00
    out dx, al

    mov dx, 0x3CE
    mov al, 0x08            ; Bit Mask = 0xFF (all bits pass through)
    out dx, al
    inc dx
    mov al, 0xFF
    out dx, al

    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret


; ── Pass 2: color fill ────────────────────────────────────────────────────────
; For each byte in the sprite bounding box:
;   Read Mode 1 returns bitmask of pixels where plane 3 = 1 (background pixels).
;   Invert → bitmask of sprite pixels (plane 3 = 0 from pass 1 or fresh clear).
;   Set GC Bit Mask to this inverted value.
;   Write 0xFF through OR ALU to planes 1,2 — fills sprite color into sprite pixels.
;
; On a black screen (plane 3=0 everywhere) the inverted mask is always 0xFF,
; so every pixel in the bounding box is treated as a sprite pixel = noise.
do_pass2:
    push ax
    push bx
    push cx
    push dx
    push di

    ; Sequencer Map Mask = 0x06 (write planes 1 and 2 only)
    ; Sprite color: planes 1,2 set → color index 0b0110 = 6 (interior, if noise)
    ;   or plane 0 also set from pass 1 → color index 0b0111 = 7 (border pixels)
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    inc dx
    mov al, 0x06
    out dx, al

    ; GC Mode = 0x08 (Read Mode 1 enabled; Write Mode 0)
    mov dx, 0x3CE
    mov al, 0x05
    out dx, al
    inc dx
    mov al, 0x08
    out dx, al

    ; GC Data Rotate = 0x10 (ALU function = OR)
    mov dx, 0x3CE
    mov al, 0x03
    out dx, al
    inc dx
    mov al, 0x10
    out dx, al

    ; GC Color Compare = 0x08 (compare value: plane 3 = 1)
    mov dx, 0x3CE
    mov al, 0x02
    out dx, al
    inc dx
    mov al, 0x08
    out dx, al

    ; GC Color Don't Care = 0x08 (only plane 3 participates in the comparison)
    mov dx, 0x3CE
    mov al, 0x07
    out dx, al
    inc dx
    mov al, 0x08
    out dx, al

    mov di, SPRITE_OFFSET
    mov bx, SPRITE_H

.row_loop:
    mov cx, SPRITE_W

.byte_loop:
    mov al, [es:di]         ; Read Mode 1: al = color-compare result
                            ;   bit=1 where plane 3=1 (background)
                            ;   bit=0 where plane 3=0 (sprite from pass 1, or all pixels if black screen)
    not al                  ; invert: bit=1 = sprite pixel (to be filled)

    ; GC Bit Mask = inverted compare (restrict write to sprite pixels only)
    mov ah, al
    mov dx, 0x3CE
    mov al, 0x08
    out dx, al
    inc dx
    mov al, ah
    out dx, al

    ; Write 0xFF: OR ALU fills planes 1,2 with 1s for every pixel in the mask.
    ; Pixels outside the mask retain their latch values unchanged.
    mov al, 0xFF
    mov [es:di], al

    inc di
    loop .byte_loop

    add di, BYTES_PER_ROW - SPRITE_W
    dec bx
    jnz .row_loop

    ; Restore GC registers to defaults
    mov dx, 0x3CE
    mov al, 0x05            ; Mode = 0x00 (Read Mode 0, no ALU)
    out dx, al
    inc dx
    mov al, 0x00
    out dx, al

    mov dx, 0x3CE
    mov al, 0x03            ; Data Rotate = 0x00 (replace, no ALU)
    out dx, al
    inc dx
    mov al, 0x00
    out dx, al

    mov dx, 0x3CE
    mov al, 0x02            ; Color Compare = 0x00
    out dx, al
    inc dx
    mov al, 0x00
    out dx, al

    mov dx, 0x3CE
    mov al, 0x07            ; Color Don't Care = 0x0F (default: all planes)
    out dx, al
    inc dx
    mov al, 0x0F
    out dx, al

    mov dx, 0x3CE
    mov al, 0x08            ; Bit Mask = 0xFF
    out dx, al
    inc dx
    mov al, 0xFF
    out dx, al

    ; Restore Map Mask to all planes
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    inc dx
    mov al, 0x0F
    out dx, al

    pop di
    pop dx
    pop cx
    pop bx
    pop ax
    ret


; ── Sprite glyph data ─────────────────────────────────────────────────────────
; 8 bytes wide × 16 rows = a hollow rectangle outline.
; Interior bytes are 0x00 (no glyph pixels). In the noise case, pass 2 fills
; the interior anyway (plane 3=0 for all pixels). In the correct case, the
; interior background color (plane 3=1) shows through unchanged.
sprite_glyph:
    db 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF  ; row  0: top border (solid)
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row  1: left/right edges
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row  2
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row  3
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row  4
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row  5
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row  6
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row  7
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row  8
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row  9
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row 10
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row 11
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row 12
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row 13
    db 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01  ; row 14
    db 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF  ; row 15: bottom border (solid)
