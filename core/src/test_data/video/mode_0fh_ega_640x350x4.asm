; EGA Graphics Mode 0x0F Test
; 640x350, Monochrome (2 active planes: 0 and 2, giving 4 intensity levels)
; Each byte covers 8 pixels; 80 bytes per row
; Plane 0 = video bit, Plane 2 = intensity bit
; 4 bands x 20 bytes (160 pixels) each = full 640-pixel width
; Band pixel value = (plane2_bit<<1) | plane0_bit -> 4 levels (0-3)

[CPU 8086]
org 0x100

start:
    ; Switch to EGA mode 0x0F (640x350, monochrome)
    mov ah, 0x00
    mov al, 0x0F
    int 0x10

    ; Set up video segment
    mov ax, 0xA000
    mov es, ax

    ; Draw 4 vertical bands, each 20 bytes (160 pixels) wide x 280 rows tall
    ; Band 0: col  0-19, mask=0x00 (black - skip, already cleared)
    ; Band 1: col 20-39, mask=0x01 (plane 0 only -> dim)
    ; Band 2: col 40-59, mask=0x04 (plane 2 only -> normal)
    ; Band 3: col 60-79, mask=0x05 (planes 0+2   -> bright)
    mov byte [band_idx], 1          ; Start at band 1 (band 0 is black, already clear)
    mov word [band_col], 20
.band_loop:
    xor ah, ah
    mov al, [band_idx]
    mov si, ax
    mov al, [band_masks + si]
    mov [cur_mask], al
    call draw_band
    inc byte [band_idx]
    add word [band_col], 20
    cmp byte [band_idx], 4
    jb .band_loop

    ; Restore map mask to all planes for BIOS text output
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    mov dx, 0x3C5
    mov al, 0x0F
    out dx, al

    ; Label each band using AH=0Eh
    ; Mode 0Fh char cell: 8x14 pixels -> char cols 0-79, char rows 0-24
    ; Each band is 20 char cols wide; label at col 8 within each band

    mov byte [label_idx], 0
    mov byte [label_col], 8
.label_loop:
    mov ah, 0x02
    mov bh, 0
    mov dh, 15                      ; Char row 15 (pixel row 210)
    mov dl, [label_col]
    int 0x10
    ; Write label char
    xor ah, ah
    mov al, [label_idx]
    mov si, ax
    shl si, 1                       ; 2 chars per label
    mov al, [band_labels + si]
    mov ah, 0x0E
    mov bh, 0
    mov bl, 15
    int 0x10
    xor ah, ah
    mov al, [label_idx]
    mov si, ax
    shl si, 1
    mov al, [band_labels + si + 1]
    mov ah, 0x0E
    mov bh, 0
    mov bl, 15
    int 0x10
    add byte [label_col], 20
    inc byte [label_idx]
    cmp byte [label_idx], 4
    jb .label_loop

    ; Print header (char row 11)
    mov ah, 0x02
    mov bh, 0
    mov dh, 11
    mov dl, 23
    int 0x10
    mov si, msg_header
    mov bl, 15
    call print_string

    ; Print info (char row 12)
    mov ah, 0x02
    mov bh, 0
    mov dh, 12
    mov dl, 24
    int 0x10
    mov si, msg_info
    mov bl, 15
    call print_string

    ; Wait for keypress
    mov ah, 0x00
    int 0x16

    ; Return to text mode
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    ; Exit
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

; Print null-terminated string using BIOS teletype (AH=0Eh)
; Input: SI = pointer to string, BL = color
print_string:
    push ax
    push bx
    push cx
    push dx
    push si
    push di
.loop:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0E
    int 0x10
    jmp .loop
.done:
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Draw a 20-byte wide, 280-row tall vertical band
; Uses cur_mask for the sequencer map mask, band_col for start column
draw_band:
    push ax
    push bx
    push cx
    push dx
    push si
    push di

    ; Clear planes 0 and 2 in this band area first
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    mov dx, 0x3C5
    mov al, 0x05                ; Planes 0 and 2 only
    out dx, al

    mov cx, 280
    mov si, 35                  ; Start at row 35 (leave top for header)
.clear_loop:
    push cx
    mov ax, si
    mov bx, 80
    mul bx
    add ax, [band_col]
    mov di, ax
    mov cx, 20
    xor al, al
.clear_inner:
    mov [es:di], al
    inc di
    loop .clear_inner
    inc si
    pop cx
    loop .clear_loop

    ; Set map mask to the desired plane combination
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    mov dx, 0x3C5
    mov al, [cur_mask]
    out dx, al

    ; Skip fill if mask is 0 (black band already cleared)
    test al, al
    jz .skip_draw

    mov cx, 280
    mov si, 35
.row_loop:
    push cx
    mov ax, si
    mov bx, 80
    mul bx
    add ax, [band_col]
    mov di, ax
    mov cx, 20
    mov al, 0xFF
.write_loop:
    mov [es:di], al
    inc di
    loop .write_loop
    inc si
    pop cx
    loop .row_loop

.skip_draw:
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Data
band_col  dw 0
band_idx  db 0
cur_mask  db 0
label_idx db 0
label_col db 0

; Map mask for each band (plane combination)
; Band 0: 0x00 (black, skipped), Band 1: 0x01 (plane 0), Band 2: 0x04 (plane 2), Band 3: 0x05 (both)
band_masks db 0x00, 0x01, 0x04, 0x05

; 2-char labels for each band
band_labels db " 0", " 1", " 2", " 3"

; Messages
msg_header db "EGA Mode 0x0F - Monochrome", 0
msg_info   db "640x350, Planes 0+2, 4 Levels", 0
