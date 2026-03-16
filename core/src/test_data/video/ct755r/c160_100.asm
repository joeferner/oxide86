; CGA 160x100 16-color mode test
; Ported from CT755r/CT.ASM - c160_100 procedure, standalone COM version.
; Uses direct 6845 register manipulation to set 160x100 mode.
; Image data is embedded via incbin from orig/C160100D.IMG.

[CPU 8086]
org 0x100
BITS 16

start:
    ; Save current video mode
    mov ah, 0x0F
    int 0x10
    mov [init_video_mode], al

    ; Set CGA color select register to 0
    mov dx, 0x3D9
    mov al, 0x00
    out dx, al

    ; Disable video signal (bit0=text mode, bit3=video enable; clear bit3)
    mov dx, 0x3D8
    mov al, 0x01
    out dx, al

    ; Program 6845 registers for 160x100 mode
    mov dx, 0x3D4           ; 6845 index port
    mov si, c160x100_param
    mov cx, 14              ; 14 register pairs
.write_6845_loop:
    lodsb                   ; AL = register index
    mov ah, [si]            ; AH = register value
    inc si
    call write_6845
    loop .write_6845_loop

    ; Enable video signal (bit0=text, bit3=video enable)
    mov dx, 0x3D8
    mov al, 0x09
    out dx, al

    ; Copy image data to CGA VRAM at 0xB800:0000
    ; 8000 words = 16000 bytes (160x100 character+attribute pairs)
    mov ax, 0xB800
    mov es, ax
    xor di, di
    mov si, image_data
    mov cx, 8000
    cld
    rep movsw

    ; Wait for keypress
    call wait_key

    ; Restore initial video mode
    xor ah, ah
    mov al, [init_video_mode]
    int 0x10

    ; sync_fix (EGA installation check, suppresses sync issue on G3101)
    mov ah, 0x12
    mov bx, 0xFF10
    int 0x10

    ; Exit
    mov ax, 0x4C00
    int 0x21

; ============================================================
; write_6845 - write value to 6845 register
; AL = register index, AH = value, DX = index port (0x3D4)
; ============================================================
write_6845:
    push dx
    cli
    out dx, al
    inc dx
    mov al, ah
    out dx, al
    sti
    pop dx
    ret

; ============================================================
; wait_key - flush keyboard buffer and wait for keypress
; ============================================================
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

; ============================================================
; Data
; ============================================================
init_video_mode     db 0

; 6845 register parameters for 160x100 mode
; Format: pairs of (register_index, value)
c160x100_param:
    db 0x00, 113    ; horizontal total
    db 0x01,  80    ; horizontal displayed
    db 0x02,  90    ; horizontal sync pos
    db 0x03,  10    ; horizontal sync width
    db 0x04, 127    ; vertical total (127 max, else no hor sync)
    db 0x05,   6    ; vertical total adjust
    db 0x06, 100    ; vertical displayed
    db 0x07, 112    ; vertical sync pos
    db 0x08,   2    ; interlace mode
    db 0x09,   1    ; max scan line (1 = 2 scan lines per row)
    db 0x0A, 0x26   ; cursor start + hide cursor (bit5)
    db 0x0B,   7    ; cursor end
    db 0x0C,   0    ; start address H
    db 0x0D,   0    ; start address L

image_data:
    incbin "orig/C160100D.IMG"
