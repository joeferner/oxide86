; INT 10h / AH=10h / AL=03h: Blink/Intensity Toggle Test
;
; Validates that:
;   BL=1 -> BLINK MODE (default): attr bit 7 = blink, bg = bits 4-6 (8 colors)
;   BL=0 -> INTENSITY MODE:       attr bit 7 = high-bg bit, bg = bits 4-7 (16 colors)
;
; Two rows of 8 color bars are drawn to B8000 directly:
;   Row A: attrs 0x0F, 0x1F, 0x2F ... 0x7F  (bit7=0, bg=0..7)
;   Row B: attrs 0x8F, 0x9F, 0xAF ... 0xFF  (bit7=1, bg bits 4-6 = 0..7)
;
; In BLINK MODE:    Row B backgrounds == Row A (bit 7 only controls blink, ignored here)
; In INTENSITY MODE: Row B backgrounds are bright versions (8-15), clearly different from A
;
; Run: cargo run -p oxide86-native-gui -- test-programs/video/blink_intensity_toggle.com

[CPU 8086]
org 0x100

start:
    ; Set 80x25 text mode (also resets to blink mode per BIOS spec)
    xor ah, ah
    mov al, 3
    int 0x10

    ; Hide cursor (scanlines 32-0 = invisible)
    mov ah, 1
    mov cx, 0x2000
    int 0x10

    ; Start in blink mode
    mov ah, 0x10
    mov al, 0x03
    mov bx, 0x0001      ; BL=1 = blink mode
    int 0x10
    mov byte [blink_mode], 1

.main_loop:
    call draw_screen
    mov ah, 0           ; INT 16h AH=00h: blocking read key
    int 0x16
    cmp al, 27          ; ESC -> exit
    je .exit
    ; Toggle mode
    xor byte [blink_mode], 1
    mov ah, 0x10
    mov al, 0x03
    xor bx, bx
    mov bl, [blink_mode]
    int 0x10
    jmp .main_loop

.exit:
    ; Restore: blink mode, visible cursor, clean text mode
    mov ah, 0x10
    mov al, 0x03
    mov bx, 0x0001
    int 0x10
    mov ah, 1
    mov cx, 0x0607
    int 0x10
    xor ah, ah
    mov al, 3
    int 0x10
    mov ah, 0x4C
    xor al, al
    int 0x21

; =============================================================
; draw_screen: clear and redraw everything
; =============================================================
draw_screen:
    push ax
    push bx
    push cx
    push dx
    push si
    push di
    push es

    ; Point ES to text video RAM
    mov ax, 0xB800
    mov es, ax

    ; Clear screen: fill 80x25 cells with 0x0720 (space, attr=white-on-black)
    xor di, di
    mov cx, 80 * 25
    mov ax, 0x0720
    rep stosw

    ; --- Title row 0 ---
    mov ah, 2           ; INT 10h AH=02h set cursor
    xor bh, bh
    xor dx, dx          ; row=0, col=0
    int 0x10
    mov ah, 9           ; INT 21h AH=09h write $-terminated string
    mov dx, title_str
    int 0x21

    ; --- Status row 2 ---
    mov ah, 2
    xor bh, bh
    mov dh, 2
    xor dl, dl
    int 0x10
    cmp byte [blink_mode], 1
    je .status_blink
    mov dx, status_intensity
    jmp .do_status
.status_blink:
    mov dx, status_blink
.do_status:
    mov ah, 9
    int 0x21

    ; --- Prompt row 3 ---
    mov ah, 2
    xor bh, bh
    mov dh, 3
    xor dl, dl
    int 0x10
    mov ah, 9
    mov dx, prompt_str
    int 0x21

    ; --- Section A label row 5 ---
    mov ah, 2
    xor bh, bh
    mov dh, 5
    xor dl, dl
    int 0x10
    mov ah, 9
    mov dx, label_a
    int 0x21

    ; --- Draw 8 bars at row 6: bit7=0, bg=0..7
    ; B8000 offset for row 6 = 6 * 160 = 960
    ; Each bar: 10 cells (20 bytes). bar i starts at offset 960 + i*20
    ; attr = (i << 4) | 0x0F  (white fg, bg=i, no blink)
    mov cl, 4           ; shift count (used throughout)
    mov si, 0           ; bar index 0..7
    mov di, 960         ; B8000 offset for row 6, col 0
.bars_a:
    cmp si, 8
    jge .bars_a_done
    mov ax, si
    shl ax, cl          ; ax = i << 4 (0x00, 0x10, ... 0x70; fits in AL since max=0x70)
    or al, 0x0F         ; al = attr
    mov ah, al          ; ah = attr (char in AL below)
    mov al, 0x20        ; space character
    push cx
    mov cx, 10
.fill_a:
    stosw
    loop .fill_a
    pop cx
    inc si
    jmp .bars_a
.bars_a_done:

    ; --- Index labels below row A, at row 7 ---
    mov ah, 2
    xor bh, bh
    mov dh, 7
    xor dl, dl
    int 0x10
    mov ah, 9
    mov dx, index_a
    int 0x21

    ; --- Section B label row 9 ---
    mov ah, 2
    xor bh, bh
    mov dh, 9
    xor dl, dl
    int 0x10
    mov ah, 9
    mov dx, label_b
    int 0x21

    ; --- Draw 8 bars at row 10: bit7=1, bg bits 4-6 = 0..7
    ; B8000 offset for row 10 = 10 * 160 = 1600
    ; attr = 0x80 | (i << 4) | 0x0F
    ; BLINK MODE:     bg = i (same as section A, bit7 just flags blink)
    ; INTENSITY MODE: bg = i + 8 (bright counterpart: 8,9,...,15)
    mov si, 0
    mov di, 1600        ; row 10, col 0
.bars_b:
    cmp si, 8
    jge .bars_b_done
    mov ax, si
    shl ax, cl          ; ax = i << 4
    or al, 0x8F         ; al = 0x80 | (i<<4) | 0x0F  (bit7 set, white fg)
    mov ah, al
    mov al, 0x20
    push cx
    mov cx, 10
.fill_b:
    stosw
    loop .fill_b
    pop cx
    inc si
    jmp .bars_b
.bars_b_done:

    ; --- Index labels below row B, at row 11 ---
    mov ah, 2
    xor bh, bh
    mov dh, 11
    xor dl, dl
    int 0x10
    mov ah, 9
    cmp byte [blink_mode], 1
    je .blink_index
    mov dx, index_b_intensity
    jmp .do_index_b
.blink_index:
    mov dx, index_b_blink
.do_index_b:
    int 0x21

    ; --- Explanation rows 13-14 ---
    mov ah, 2
    xor bh, bh
    mov dh, 13
    xor dl, dl
    int 0x10
    mov ah, 9
    cmp byte [blink_mode], 1
    je .blink_explain
    mov dx, explain_intensity
    jmp .do_explain
.blink_explain:
    mov dx, explain_blink
.do_explain:
    int 0x21

    pop es
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; =============================================================
; Data
; =============================================================
blink_mode      db 1        ; 1=blink (default), 0=intensity

title_str       db 'INT 10h/AH=10h/AL=03h Blink/Intensity Toggle', 13, 10, '$'

status_blink    db 'Status: BLINK MODE    (BL=1) - bit7=blink,  bg uses bits 4-6 (0-7) ', '$'
status_intensity db 'Status: INTENSITY MODE (BL=0) - bit7=hi-bg, bg uses bits 4-7 (0-15)', '$'

prompt_str      db 'Press SPACE to toggle mode, ESC to exit', '$'

label_a         db 'Row A - bit7=0 : standard backgrounds 0-7:', '$'
label_b         db 'Row B - bit7=1 : (BLINK: blinks on 0-7 / INTENSITY: bright bg 8-15):', '$'

;            col 0    col10    col20    col30    col40    col50    col60    col70
index_a         db '  bg=0     bg=1     bg=2     bg=3     bg=4     bg=5     bg=6     bg=7  $'
index_b_blink   db ' (0=blk)  (1=blu)  (2=grn)  (3=cyn)  (4=red)  (5=mag)  (6=brn)  (7=wht)$'
index_b_intensity db '  bg=8     bg=9    bg=10    bg=11    bg=12    bg=13    bg=14    bg=15  $'

explain_blink   db 'BLINK MODE:    Row B bit7=blink flag -> same bg colors as Row A (0-7)', '$'
explain_intensity db 'INTENSITY MODE: Row B bit7=hi-bg bit -> bright backgrounds 8-15       ', '$'
