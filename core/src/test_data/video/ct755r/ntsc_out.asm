; NTSC Composite Out Color test
; Ported from CT755r/CT.ASM - standalone COM version
; Uses direct CGA video memory writes instead of BIOS pixel calls

[CPU 8086]
org 0x100
BITS 16

start:
    ; Save current video mode
    mov ah, 0x0F
    int 0x10
    mov [init_video_mode], al

    ; Set CGA mode 4 (320x200, 4-color)
    xor ah, ah
    mov al, 0x04
    int 0x10

    ; Set black background, palette 0
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x00
    int 0x10

    mov ah, 0x0B
    mov bh, 0x01
    mov bl, 0x00
    int 0x10

    ; Write NTSC image to CGA video memory
    call write_screen

    ; Screen 1: mode 4, palette 0 normal
    mov dx, 0x000F
    call set_cursor
    mov si, ntsc_m4_p0_s
    call print_str
    call wait_key

    ; Screen 2: mode 4, palette 0 high intensity
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x10
    int 0x10
    mov dx, 0x000F
    call set_cursor
    mov si, ntsc_m4_p0h_s
    call print_str
    call wait_key

    ; Screen 3: mode 4, palette 1 normal
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x00
    int 0x10
    mov ah, 0x0B
    mov bh, 0x01
    mov bl, 0x01
    int 0x10
    mov dx, 0x000F
    call set_cursor
    mov si, ntsc_m4_p1_s
    call print_str
    call wait_key

    ; Screen 4: mode 4, palette 1 high intensity
    mov ah, 0x0B
    mov bh, 0x00
    mov bl, 0x10
    int 0x10
    mov dx, 0x000F
    call set_cursor
    mov si, ntsc_m4_p1h_s
    call print_str
    call wait_key

    ; Screen 5: mode 6, color on composite (direct port)
    mov dx, 0x3D8
    mov al, 0x1A
    out dx, al
    mov dx, 0x3D9
    mov al, 0x07
    out dx, al
    mov dx, 0x000F
    call set_cursor
    mov si, ntsc_m6_s
    call print_str
    call wait_key

    ; Screen 6: mode 6, high intensity
    mov dx, 0x3D9
    mov al, 0x0F
    out dx, al
    mov dx, 0x000F
    call set_cursor
    mov si, ntsc_m6h_s
    call print_str
    call wait_key

    ; Restore video mode
    xor ah, ah
    mov al, [init_video_mode]
    int 0x10

    ; sync_fix (EGA installation check, suppresses sync issue on G3101)
    mov ah, 0x12
    mov bx, 0xFF10
    int 0x10

    mov ah, 0x4C
    xor al, al
    int 0x21

;--- write_screen ---
; Writes ntsc_data (sequential rows) to CGA video memory at 0xB800.
; CGA mode 4: even rows at bank 0 (0x0000), odd rows at bank 1 (0x2000).
; Each row is 80 bytes (320 pixels / 4 per byte).
write_screen:
    push es
    push ax
    push bx
    push cx
    push si
    push di

    mov ax, 0xB800
    mov es, ax
    mov si, ntsc_data
    xor cx, cx              ; row = 0

.ws_row:
    mov ax, cx
    test al, 1
    jnz .ws_odd

    shr ax, 1
    mov bx, 80
    mul bx
    mov di, ax
    jmp .ws_copy

.ws_odd:
    dec ax
    shr ax, 1
    mov bx, 80
    mul bx
    add ax, 0x2000
    mov di, ax

.ws_copy:
    push cx
    mov cx, 40              ; 80 bytes = 40 words
    rep movsw
    pop cx
    inc cx
    cmp cx, 200
    jb .ws_row

    pop di
    pop si
    pop cx
    pop bx
    pop ax
    pop es
    ret

;--- wait_key ---
wait_key:
    push es
    push ax
    cli
    mov ax, 0x40
    mov es, ax
    mov al, [es:0x1A]
    mov [es:0x1C], al       ; flush keyboard buffer
    sti
    pop ax
    pop es
    xor ah, ah
    int 0x16
    ret

;--- print_str ---
; SI = pointer to null-terminated string
print_str:
    push ax
    push bx
.ps_loop:
    lodsb
    cmp al, 0
    je .ps_done
    mov bx, 0x0001
    mov ah, 0x0E
    int 0x10
    jmp .ps_loop
.ps_done:
    pop bx
    pop ax
    ret

;--- set_cursor ---
; DX: DH=row, DL=col
set_cursor:
    push ax
    push bx
    mov ah, 0x02
    mov bh, 0x00
    int 0x10
    pop bx
    pop ax
    ret

;--- data ---
init_video_mode db 0x00

ntsc_m4_p0_s  db "NTSC mode 4, Palette 0   ", 0
ntsc_m4_p0h_s db "NTSC mode 4, Palette 0 HI", 0
ntsc_m4_p1_s  db "NTSC mode 4, Palette 1   ", 0
ntsc_m4_p1h_s db "NTSC mode 4, Palette 1 HI", 0
ntsc_m6_s     db "NTSC mode 6              ", 0
ntsc_m6h_s    db "NTSC mode 6 HI           ", 0

ntsc_data:
    incbin "NTSCDATA.IMG"
