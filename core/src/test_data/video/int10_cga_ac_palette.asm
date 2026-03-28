; INT 10h CGA mode 04h AC palette + DAC routing test
;
; Mirrors the palette setup seen in Checkit's Graphics Grid Test:
;
;   1. Set video mode 0x04 (320x200 4-color)
;   2. AH=0Bh: set border/background color = 0x20
;      -> After this: palette=true, intensity=false -> color_map[3]=7 (DAC 7)
;   3. AH=10h AL=02h: program AC palette registers (pixel i -> DAC i)
;   4. AH=10h AL=10h: set DAC registers:
;        DAC 0 = RGB(  0,  0,  0)  black
;        DAC 1 = RGB( 21, 63, 63)  cyan
;        DAC 2 = RGB( 63, 21, 63)  magenta
;        DAC 3 = RGB( 63, 63, 63)  white  <- correct target via AC palette
;        DAC 7 = RGB( 63, 21, 63)  magenta <- wrong: what color_map[3] resolves to
;   5. Write the word "WHITE" at the screen center using AH=09h with BL=0x03
;      (fg color 3 -> should resolve via AC[3]=3 -> DAC 3 -> white)
;
; Without AC palette fix: pixel 3 -> color_map[3]=7 -> DAC 7 = magenta (purple)
; With AC palette fix:    pixel 3 -> AC[3]=3 -> DAC 3 = white (correct)
;
; Expected screenshot (after fix): "WHITE" in white on a black background.

[CPU 8086]
org 0x100

start:
    ; ── 1. Set video mode 04h ────────────────────────────────────────────────
    mov ah, 0x00
    mov al, 0x04
    int 0x10

    ; ── 2. Set border/background color = 0x20 ────────────────────────────────
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x20
    int 0x10

    ; ── 3. Program AC palette registers (AL=02h) ─────────────────────────────
    ; ES:DX -> 17-byte table: 16 AC register values + 1 border byte.
    ; AC[i] = i  ->  pixel value i maps to DAC register i.
    ; border = 16  (matches Checkit log "border=16")
    mov ah, 0x10
    mov al, 0x02
    push cs
    pop es
    mov dx, ac_table
    int 0x10

    ; ── 4. Set DAC registers with Checkit values ──────────────────────────────
    ; DAC 0 = black   RGB(0, 0, 0)
    mov ah, 0x10
    mov al, 0x10
    xor bx, bx
    xor dh, dh      ; red
    xor ch, ch      ; green
    xor cl, cl      ; blue
    int 0x10

    ; DAC 1 = cyan    RGB(21, 63, 63)
    mov ah, 0x10
    mov al, 0x10
    mov bx, 1
    mov dh, 21      ; red
    mov ch, 63      ; green
    mov cl, 63      ; blue
    int 0x10

    ; DAC 2 = magenta RGB(63, 21, 63)
    mov ah, 0x10
    mov al, 0x10
    mov bx, 2
    mov dh, 63      ; red
    mov ch, 21      ; green
    mov cl, 63      ; blue
    int 0x10

    ; DAC 3 = white   RGB(63, 63, 63)
    mov ah, 0x10
    mov al, 0x10
    mov bx, 3
    mov dh, 63      ; red
    mov ch, 63      ; green
    mov cl, 63      ; blue
    int 0x10

    ; DAC 7 = magenta (simulates Checkit's AL=12h block write which corrupts this slot)
    ; After AH=0Bh sets color_select=0x20 (palette=true, intensity=false),
    ; color_map[3] = 7. Without AC palette fix, pixel 3 -> DAC 7 = magenta (wrong).
    ; With fix: pixel 3 -> AC[3]=3 -> DAC 3 -> white (correct).
    mov ah, 0x10
    mov al, 0x10
    mov bx, 7
    mov dh, 63      ; red
    mov ch, 21      ; green
    mov cl, 63      ; blue
    int 0x10

    ; ── Hide hardware cursor ──────────────────────────────────────────────────
    mov ah, 0x01
    mov cx, 0x2000
    int 0x10

    ; ── 5. Write "WHITE" at row 12, col 17 (center of 40-col screen) ─────────
    ; Use AH=09h (write char+attr) with BL=0x03 (opaque, fg=3=white).
    ; AH=09h does not advance the cursor, so we track the column in [cur_col].
    ; BX must not be used as the column counter here because mov bl, 0x03
    ; (the color for AH=09h) would clobber it.
    mov byte [cur_col], 17
    mov si, msg
.write_loop:
    mov al, [si]
    test al, al
    jz .done

    ; Set cursor position: row=12, col=[cur_col], page=0
    mov ah, 0x02
    mov bh, 0
    mov dh, 12
    mov dl, [cur_col]
    int 0x10

    ; Write character: AL=char, BH=page, BL=fg color (3=white), CX=1
    mov ah, 0x09
    mov bh, 0
    mov bl, 0x03        ; opaque, fg=3
    mov cx, 1
    int 0x10

    inc byte [cur_col]
    inc si
    jmp .write_loop

.done:
    ; Wait for keypress -> caller takes screenshot
    mov ah, 0x00
    int 0x16

    ; Return to text mode and exit
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

; ── Data ─────────────────────────────────────────────────────────────────────
cur_col db 0
msg db "WHITE", 0

; AC palette table for INT 10h AH=10h AL=02h.
; 16 bytes: AC register values (pixel i -> DAC i), then 1 border byte.
ac_table:
    db 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15
    db 16       ; border = 16  (matches Checkit log "border=16")
