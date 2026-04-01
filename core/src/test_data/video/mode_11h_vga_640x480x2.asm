; VGA Graphics Mode 0x11 Test
; 640x480, 2 Colors (monochrome, plane 0 only at A000:0000)
; Each byte covers 8 pixels; 80 bytes per row
; Displays black/white pattern with labels
; Tests: direct planar writes, AH=0Eh (transparent teletype)

[CPU 8086]
org 0x100

start:
    ; Switch to VGA mode 0x11 (640x480, 2 colors)
    mov ah, 0x00
    mov al, 0x11
    int 0x10

    ; Set up video segment for direct memory access
    mov ax, 0xA000
    mov es, ax

    ; Draw a white rectangle (left half, top portion)
    ; 40 bytes wide (320 pixels) x 160 rows tall, starting at row 0
    mov dx, 0x3C4
    mov al, 0x02            ; Select Map Mask register
    out dx, al
    mov dx, 0x3C5
    mov al, 0x01            ; Plane 0 only (mode 11h uses plane 0)
    out dx, al

    mov cx, 160             ; 160 rows
    xor si, si              ; Starting row 0
.white_left:
    push cx
    mov ax, si
    mov bx, 80
    mul bx
    mov di, ax              ; offset = row * 80
    mov cx, 40              ; 40 bytes = 320 pixels
    mov al, 0xFF            ; All pixels on (white)
.white_left_inner:
    mov [es:di], al
    inc di
    loop .white_left_inner
    inc si
    pop cx
    loop .white_left

    ; Draw a checkerboard pattern (right half, top portion)
    ; 40 bytes wide x 160 rows, starting at column byte 40
    mov cx, 160
    xor si, si              ; Starting row 0
.checker:
    push cx
    mov ax, si
    mov bx, 80
    mul bx
    add ax, 40              ; Start at byte column 40
    mov di, ax
    mov cx, 40
    ; Alternate 0xAA and 0x55 per row for checkerboard
    test si, 1
    jnz .checker_odd
    mov al, 0xAA
    jmp .checker_write
.checker_odd:
    mov al, 0x55
.checker_write:
    mov [es:di], al
    inc di
    loop .checker_write
    inc si
    pop cx
    loop .checker

    ; Draw vertical stripes (left half, bottom portion rows 320-479)
    ; Alternating 4-pixel wide stripes: 0xF0 pattern
    mov cx, 160
    mov si, 320             ; Starting row 320
.stripes:
    push cx
    mov ax, si
    mov bx, 80
    mul bx
    mov di, ax
    mov cx, 40
    mov al, 0xF0            ; 4 white, 4 black repeating
.stripes_inner:
    mov [es:di], al
    inc di
    loop .stripes_inner
    inc si
    pop cx
    loop .stripes

    ; Draw horizontal stripes (right half, bottom portion rows 320-479)
    ; Every other row is white
    mov cx, 160
    mov si, 320
.hstripes:
    push cx
    mov ax, si
    mov bx, 80
    mul bx
    add ax, 40              ; Right half
    mov di, ax
    test si, 1
    jnz .hstripes_skip      ; Odd rows stay black
    mov cx, 40
    mov al, 0xFF
.hstripes_inner:
    mov [es:di], al
    inc di
    loop .hstripes_inner
.hstripes_skip:
    inc si
    pop cx
    loop .hstripes

    ; Restore map mask to all planes for BIOS text output
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    mov dx, 0x3C5
    mov al, 0x0F
    out dx, al

    ; --- Labels using BIOS teletype AH=0Eh ---

    ; Label "SOLID" in top-left box (char row 4, col 16)
    mov ah, 0x02
    mov bh, 0
    mov dh, 4
    mov dl, 16
    int 0x10
    mov si, msg_solid
    mov bl, 15
    call print_string

    ; Label "CHECK" in top-right box (char row 4, col 56)
    mov ah, 0x02
    mov bh, 0
    mov dh, 4
    mov dl, 56
    int 0x10
    mov si, msg_check
    mov bl, 15
    call print_string

    ; Label "V-STRIPE" in bottom-left box (char row 24, col 14)
    mov ah, 0x02
    mov bh, 0
    mov dh, 24
    mov dl, 14
    int 0x10
    mov si, msg_vstripe
    mov bl, 15
    call print_string

    ; Label "H-STRIPE" in bottom-right box (char row 24, col 54)
    mov ah, 0x02
    mov bh, 0
    mov dh, 24
    mov dl, 54
    int 0x10
    mov si, msg_hstripe
    mov bl, 15
    call print_string

    ; Print header in middle gap (char row 12, centered)
    mov ah, 0x02
    mov bh, 0
    mov dh, 12
    mov dl, 22
    int 0x10
    mov si, msg_header
    mov bl, 15
    call print_string

    ; Print info line (char row 13)
    mov ah, 0x02
    mov bh, 0
    mov dh, 13
    mov dl, 23
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

; Print null-terminated string using BIOS teletype (AH=0Eh, transparent)
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

; Messages
msg_solid   db "SOLID", 0
msg_check   db "CHECK", 0
msg_vstripe db "V-STRIPE", 0
msg_hstripe db "H-STRIPE", 0
msg_header  db "VGA Mode 0x11 - 2 Colors", 0
msg_info    db "640x480, Monochrome (1bpp)", 0
