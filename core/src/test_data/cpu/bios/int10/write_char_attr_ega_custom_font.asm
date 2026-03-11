; INT 10h Function 09h - Write Character using patched INT 43h custom font
; Simulates the inverted-glyph technique used by games like King's Quest:
;   1. Reads the 'A' glyph from the BIOS ROM font at F000:FA6E
;   2. Inverts it (XOR 0xFF) to produce a dark-on-white glyph
;   3. Stores the inverted glyph in a local table and patches INT 43h to it
;   4. Draws char 0x80 (which the BIOS maps to the patched glyph) in white
; The result should look like a dark 'A' on a white background.

[CPU 8086]
org 0x0100

start:
    ; Set EGA mode 0x0D (320x200 16-colour)
    mov ah, 0x00
    mov al, 0x0D
    int 0x10

    ; Cursor to row 0, col 0
    mov ah, 0x02
    xor bx, bx
    xor dx, dx
    int 0x10

    ; --- Build inverted glyph for 'A' from the BIOS ROM font ---
    ; In a COM file ES = CS = DS initially.
    ; Set DS = F000 to read the ROM font; keep ES = program segment for writes.
    mov ax, 0xF000
    mov ds, ax
    mov si, 0xFA6E + 0x41*8     ; DS:SI → 'A' glyph in BIOS ROM

    mov di, glyph_80            ; ES:DI → our glyph buffer (ES = program segment)
    mov cx, 8
.invert_loop:
    mov al, [si]                ; read byte from F000:...
    xor al, 0xFF                ; invert: bg→fg, fg→bg
    mov [es:di], al             ; write to program segment buffer
    inc si
    inc di
    loop .invert_loop

    ; Restore DS = program segment
    mov ax, es
    mov ds, ax

    ; --- Patch INT 43h to point to our local font table ---
    ; font_base is placed so that glyph_80 = font_base + 0x80*8 = font_base + 0x400
    push es
    xor ax, ax
    mov es, ax                  ; ES = 0 (IVT segment)
    mov word [es:0x010C], font_base
    mov word [es:0x010E], cs
    pop es                      ; restore ES = program segment

    ; Draw char 0x80 using the custom (inverted 'A') glyph, colour white (0x0F)
    mov ah, 0x09
    mov al, 0x80
    xor bh, bh
    mov bl, 0x0F
    mov cx, 1
    int 0x10

    ; Wait for key – visual assertion happens here
    mov ah, 0x00
    int 0x16

    ; Verify: char (0,0) pixel row 0 = A000:0000 must be non-zero.
    ; Inverted 'A' glyph has top row = 0xFF ^ <'A' row 0>.
    ; Drawn with fg=0x0F (all planes set), so plane 0 byte = glyph_byte.
    mov ax, 0xA000
    mov es, ax
    mov al, [es:0x0000]
    cmp al, 0x00
    je fail

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

; --- Data section ---
; font_base is the INT 43h table base: glyph for char N is at font_base + N*8.
; glyph_80 must be exactly 0x80*8 = 0x400 bytes past font_base.
font_base:
    times 0x400 db 0    ; placeholder glyphs for chars 0x00-0x7F
glyph_80:
    times 8 db 0        ; filled at runtime by the invert loop above
