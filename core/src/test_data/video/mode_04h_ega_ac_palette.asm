; EGA card + CGA mode 04h: AC palette maps via EGA 6-bit color codes
;
; Mirrors the palette setup seen in CheckIt's Graphics Grid Test #1 on EGA hardware:
;
;   1. Set video mode 0x04 (320x200 4-color) on an EGA card
;   2. AH=10h AL=02h: set AC registers 0-3 to [0, 19, 21, 23]
;      (values from CheckIt log: "Set AC register 3 = 23" etc.)
;   3. AH=10h AL=00h: set AC registers 0-3 individually to same values
;   4. Write "EGA" at center using fg=3 (pixel value 3)
;
; AC[3] = 23 = 0x17 = 0b010111
;   EGA 6-bit color: r_sec=0,g_sec=1,b_sec=0, r_prim=1,g_prim=1,b_prim=1
;   -> R=42, G=63, B=42  (bright green)
;
; Without EGA DAC fix: pixel 3 -> DAC[23] = [24,24,24] = dark gray (nearly black)
; With EGA DAC fix:    pixel 3 -> DAC[23] = [42,63,42] = bright green (visible)

[CPU 8086]
org 0x100

start:
    ; ── 1. Set video mode 04h ────────────────────────────────────────────────
    mov ah, 0x00
    mov al, 0x04
    int 0x10

    ; ── 2. Program all 16 AC palette registers via AL=02h ────────────────────
    ; Table: AC[i] = i for i=0..15, border=0
    ; This sets ac_palette_programmed=true and AC[0..15] = identity mapping.
    mov ah, 0x10
    mov al, 0x02
    push cs
    pop es
    mov dx, ac_table
    int 0x10

    ; ── 3. Override AC registers 0-3 with CheckIt values ─────────────────────
    ; AC[3] = 23 (0x17) -> EGA color R=42,G=63,B=42 (bright green on EGA DAC)
    mov ah, 0x10
    mov al, 0x00
    mov bh, 23          ; value
    mov bl, 3           ; register index
    int 0x10

    mov ah, 0x10
    mov al, 0x00
    mov bh, 21          ; value
    mov bl, 2           ; register index
    int 0x10

    mov ah, 0x10
    mov al, 0x00
    mov bh, 19          ; value
    mov bl, 1           ; register index
    int 0x10

    mov ah, 0x10
    mov al, 0x00
    mov bh, 0           ; value
    mov bl, 0           ; register index
    int 0x10

    ; ── Hide hardware cursor ──────────────────────────────────────────────────
    mov ah, 0x01
    mov cx, 0x2000
    int 0x10

    ; ── 4. Write "EGA" at row 12, col 18 using fg=3 ──────────────────────────
    ; pixel value 3 -> AC[3]=23 -> DAC[23]=EGA green (with fix)
    mov byte [cur_col], 18
    mov si, msg
.write_loop:
    mov al, [si]
    test al, al
    jz .done

    mov ah, 0x02
    mov bh, 0
    mov dh, 12
    mov dl, [cur_col]
    int 0x10

    mov ah, 0x09
    mov bh, 0
    mov bl, 0x03        ; fg=3, opaque
    mov cx, 1
    int 0x10

    inc byte [cur_col]
    inc si
    jmp .write_loop

.done:
    mov ah, 0x00
    int 0x16

    mov ah, 0x00
    mov al, 0x03
    int 0x10

    mov ah, 0x4C
    xor al, al
    int 0x21

; ── Data ─────────────────────────────────────────────────────────────────────
cur_col db 0
msg     db "EGA", 0

; AC palette table for AL=02h: AC[i]=i for i=0..15, then border=0
ac_table:
    db 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15
    db 0
