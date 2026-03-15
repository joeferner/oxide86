; Tests INT 10h/AH=11h/AL=30h font info queries and direct EGA VRAM glyph rendering.
;
; Mirrors the technique SimCity uses to render menu text:
;   1. Query font pointer via AH=11h/AL=30h/BH=XX to get ES:BP, CX (bytes/char)
;   2. Compute glyph address = font_base + char_code * CX (exactly as SimCity does)
;   3. Set EGA map mask, write glyph rows directly into A000h VRAM
;
; Three rows of text are rendered, each using a different font info pointer type:
;   Pixel row  28: BH=01h (INT 43h vector)     -> 8x14 EGA font
;   Pixel row  98: BH=02h (ROM 8x14)           -> 8x14 EGA font  (SimCity path)
;   Pixel row 168: BH=03h (ROM 8x8 double-dot) -> 8x8  CGA font
;
; vram_row is an absolute pixel row so layout stays consistent regardless of char_height.
; Waits for a keypress; assert_screen is taken at that point.

[CPU 8086]
org 0x100

start:
    ; EGA mode 0x10 (640x350, 16 colors, 8x14 char cells, 80 bytes/row)
    mov ax, 0x0010
    int 0x10

    ; --- Row 0: BH=01h (INT 43h pointer) ---
    mov ax, 0x1130
    mov bh, 0x01
    int 0x10                ; ES:BP = font base, CX = bytes/char
    mov [font_seg], es
    mov [font_off], bp
    mov [char_height], cx

    mov word [vram_col], 4  ; byte column offset (col 4 = pixel 32)
    mov word [vram_row], 28 ; absolute pixel row
    mov si, msg_bh01
    call render_string

    ; --- Row 1: BH=02h (ROM 8x14 — the SimCity path) ---
    mov ax, 0x1130
    mov bh, 0x02
    int 0x10
    mov [font_seg], es
    mov [font_off], bp
    mov [char_height], cx

    mov word [vram_col], 4
    mov word [vram_row], 98 ; absolute pixel row (well clear of row 0)
    mov si, msg_bh02
    call render_string

    ; --- Row 2: BH=03h (ROM 8x8 double-dot) ---
    mov ax, 0x1130
    mov bh, 0x03
    int 0x10
    mov [font_seg], es
    mov [font_off], bp
    mov [char_height], cx

    mov word [vram_col], 4
    mov word [vram_row], 168 ; absolute pixel row (well clear of row 1)
    mov si, msg_bh03
    call render_string

    ; Wait for keypress — assert_screen is taken here
    mov ah, 0x00
    int 0x16

    ; Return to text mode and exit
    mov ax, 0x0003
    int 0x10
    mov ax, 0x4C00
    int 0x21

; Render null-terminated string; advances vram_col by 1 per character.
render_string:
    push ax
    push si
.loop:
    lodsb
    test al, al
    jz .done
    call render_char
    inc word [vram_col]
    jmp .loop
.done:
    pop si
    pop ax
    ret

; Render character AL at (vram_row, vram_col) using font_seg:font_off.
; vram_row is an absolute pixel row; vram_col is a byte column (1 byte = 8 pixels).
; White-on-black: map mask = 0x0F, glyph bytes written directly to A000h.
;
; SimCity glyph address formula (mirrored exactly):
;   glyph_offset = char_code * char_height
;   glyph_src    = font_seg:font_off + glyph_offset
render_char:
    push ax
    push bx
    push cx
    push dx
    push si
    push di
    push es
    push ds

    ; Compute glyph address: font_off + char_code * char_height
    ; (must happen before DS is changed to font_seg)
    xor ah, ah              ; zero-extend AL -> AX (char_code)
    mov bx, [char_height]
    mul bx                  ; AX = char_code * char_height
    add ax, [font_off]      ; AX = font_off + glyph_offset
    mov si, ax              ; SI = glyph offset (used after DS switch)

    ; Compute VRAM byte offset: vram_row * 80 + vram_col
    ; (must happen before DS is changed away from program segment)
    mov ax, [vram_row]      ; absolute pixel row
    mov bx, 80
    mul bx                  ; AX = vram_row * 80
    add ax, [vram_col]      ; AX = VRAM byte offset for top-left of char
    mov di, ax

    ; Load char_height into CX before switching DS away from program segment
    mov cx, [char_height]

    ; Now switch DS to the BIOS font segment; DS:SI -> glyph data
    mov ds, [font_seg]

    ; Point ES at EGA VRAM
    mov ax, 0xA000
    mov es, ax

    ; Write mode 0 in GDC (Graphics Controller)
    mov dx, 0x3CE
    mov al, 0x05            ; mode register index
    out dx, al
    inc dx
    mov al, 0x00            ; write mode 0
    out dx, al

    ; Map mask = 0x0F (all 4 planes -> white on black)
    mov dx, 0x3C4
    mov al, 0x02            ; sequencer map mask index
    out dx, al
    inc dx
    mov al, 0x0F
    out dx, al

    ; Write cx glyph rows to VRAM
.row_loop:
    lodsb                   ; glyph row byte from DS:SI
    mov [es:di], al         ; write to all 4 EGA planes simultaneously
    add di, 80              ; next scan line
    loop .row_loop

    pop ds
    pop es
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Data
vram_col    dw 0
vram_row    dw 0
char_height dw 14
font_seg    dw 0
font_off    dw 0

msg_bh01    db "BH=01 INT43: Easy Medium Hard", 0
msg_bh02    db "BH=02 ROM14: Easy Medium Hard", 0
msg_bh03    db "BH=03 ROM8x8: Easy Medium Hard", 0
