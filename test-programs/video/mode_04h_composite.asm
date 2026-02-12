; CGA Composite Mode Test
; Starts in mode 0x04 (320x200, 4 colors), then enables composite via port 0x3D8
; This creates a 640x200 mode with NTSC composite artifact coloring
; Each byte = 2 nibbles, each nibble (0-15) maps to a color palette entry

org 0x100

start:
    ; Switch to CGA mode 0x04 (320x200, 4 colors)
    mov ah, 0x00
    mov al, 0x04
    int 0x10

    ; Set up video segment for direct memory access
    mov ax, 0xB800
    mov es, ax

    ; Draw some patterns in 2bpp format (mode 0x04 format)
    ; These will be reinterpreted as nibbles when composite mode is enabled

    ; Fill first few rows with gradient patterns
    ; Row 0: 0x00 (nibbles 0,0 = black, black)
    mov di, 0
    mov cx, 80
    mov al, 0x00
    rep stosb

    ; Row 1: 0x11 (nibbles 1,1)
    mov cx, 80
    mov al, 0x11
    rep stosb

    ; Row 2: 0x22 (nibbles 2,2)
    mov cx, 80
    mov al, 0x22
    rep stosb

    ; Row 3: 0x33 (nibbles 3,3)
    mov cx, 80
    mov al, 0x33
    rep stosb

    ; Row 4: 0x44 (nibbles 4,4)
    mov cx, 80
    mov al, 0x44
    rep stosb

    ; Row 5: 0x55 (nibbles 5,5)
    mov cx, 80
    mov al, 0x55
    rep stosb

    ; Row 6: 0x66 (nibbles 6,6)
    mov cx, 80
    mov al, 0x66
    rep stosb

    ; Row 7: 0x77 (nibbles 7,7)
    mov cx, 80
    mov al, 0x77
    rep stosb

    ; Row 8: 0x88 (nibbles 8,8)
    mov cx, 80
    mov al, 0x88
    rep stosb

    ; Row 9: 0x99 (nibbles 9,9)
    mov cx, 80
    mov al, 0x99
    rep stosb

    ; Row 10: 0xAA (nibbles A,A)
    mov cx, 80
    mov al, 0xAA
    rep stosb

    ; Row 11: 0xBB (nibbles B,B)
    mov cx, 80
    mov al, 0xBB
    rep stosb

    ; Row 12: 0xCC (nibbles C,C)
    mov cx, 80
    mov al, 0xCC
    rep stosb

    ; Row 13: 0xDD (nibbles D,D)
    mov cx, 80
    mov al, 0xDD
    rep stosb

    ; Row 14: 0xEE (nibbles E,E)
    mov cx, 80
    mov al, 0xEE
    rep stosb

    ; Row 15: 0xFF (nibbles F,F)
    mov cx, 80
    mov al, 0xFF
    rep stosb

    ; Draw some vertical bars in lower half (rows 100-139)
    ; Each bar uses different nibble patterns
    call draw_vertical_bars

    ; Start with composite mode OFF
    mov byte [composite_enabled], 0

    ; Main loop: toggle composite mode on each keypress
.main_loop:
    ; Clear the message area (rows 22-23) by drawing black pixels
    call clear_message_area

    ; Print first line of message (row 22)
    mov ah, 0x02
    mov bh, 0
    mov dh, 22          ; Row 22
    mov dl, 1           ; Column 1
    int 0x10

    ; Print appropriate message based on composite state
    cmp byte [composite_enabled], 0
    je .show_enable_msg

    ; Composite is ON - show disable message line 1
    mov si, msg_composite_on_1
    mov bl, 3           ; Cyan color
    call print_string

    ; Print line 2 at row 23
    mov ah, 0x02
    mov bh, 0
    mov dh, 23
    mov dl, 1
    int 0x10

    mov si, msg_composite_on_2
    mov bl, 3
    call print_string
    jmp .wait_key

.show_enable_msg:
    ; Composite is OFF - show enable message line 1
    mov si, msg_composite_off_1
    mov bl, 3           ; Cyan color
    call print_string

    ; Print line 2 at row 23
    mov ah, 0x02
    mov bh, 0
    mov dh, 23
    mov dl, 1
    int 0x10

    mov si, msg_composite_off_2
    mov bl, 3
    call print_string

.wait_key:
    ; Wait for keypress
    mov ah, 0x00
    int 0x16

    ; Check if ESC was pressed (scan code 0x01)
    cmp ah, 0x01
    je .exit

    ; Toggle composite mode
    mov dx, 0x3D8
    cmp byte [composite_enabled], 0
    je .enable_composite

    ; Disable composite mode
    mov al, 0x0A        ; Normal mode 0x04 (no hires bit)
    out dx, al
    mov byte [composite_enabled], 0
    jmp .main_loop

.enable_composite:
    ; Enable composite mode
    mov al, 0x1A        ; Enable hires bit for composite mode
    out dx, al
    mov byte [composite_enabled], 1
    jmp .main_loop

.exit:
    ; Return to text mode
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    ; Exit
    mov ah, 0x4C
    int 0x21

; Print null-terminated string using BIOS teletype
; Input: SI = pointer to string, BL = color
print_string:
    push ax
    push bx
    push si
.loop:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0E        ; Teletype output
    mov bh, 0           ; Page 0 (must be set explicitly)
    int 0x10
    jmp .loop
.done:
    pop si
    pop bx
    pop ax
    ret

; Clear message area (rows 22-23, which is pixel rows 176-191)
; Clears by writing black pixels directly to video memory
clear_message_area:
    push ax
    push bx
    push cx
    push di
    push es

    mov ax, 0xB800
    mov es, ax

    ; Clear 16 pixel rows (176-191) for character rows 22-23
    mov bx, 176         ; Starting pixel row

.row_loop:
    ; Calculate offset for current row
    mov ax, bx
    test al, 1          ; Check if odd
    jz .even_row

    ; Odd row: 0x2000 + ((row-1)/2) * 80
    dec ax
    shr ax, 1
    mov cx, 80
    mul cx
    add ax, 0x2000
    mov di, ax
    jmp .clear_row

.even_row:
    ; Even row: (row/2) * 80
    shr ax, 1
    mov cx, 80
    mul cx
    mov di, ax

.clear_row:
    ; Clear 80 bytes for this row
    push bx
    mov cx, 80
    xor al, al          ; Black pixels
    rep stosb
    pop bx

    ; Move to next row
    inc bx
    cmp bx, 192         ; Stop after row 191
    jl .row_loop

    pop es
    pop di
    pop cx
    pop bx
    pop ax
    ret

; Draw vertical bars with different nibble patterns
; Rows 100-139 (40 rows), 8 bars of 10 bytes each
draw_vertical_bars:
    pusha

    ; Calculate starting offset for row 100
    ; Row 100 is even, so: (100/2) * 80 = 50 * 80 = 4000
    mov di, 4000

    mov cx, 40          ; 40 rows

.row_loop:
    push cx
    push di

    ; Bar 1: 0x01 (nibbles 0,1)
    mov cx, 10
    mov al, 0x01
    rep stosb

    ; Bar 2: 0x23 (nibbles 2,3)
    mov cx, 10
    mov al, 0x23
    rep stosb

    ; Bar 3: 0x45 (nibbles 4,5)
    mov cx, 10
    mov al, 0x45
    rep stosb

    ; Bar 4: 0x67 (nibbles 6,7)
    mov cx, 10
    mov al, 0x67
    rep stosb

    ; Bar 5: 0x89 (nibbles 8,9)
    mov cx, 10
    mov al, 0x89
    rep stosb

    ; Bar 6: 0xAB (nibbles A,B)
    mov cx, 10
    mov al, 0xAB
    rep stosb

    ; Bar 7: 0xCD (nibbles C,D)
    mov cx, 10
    mov al, 0xCD
    rep stosb

    ; Bar 8: 0xEF (nibbles E,F)
    mov cx, 10
    mov al, 0xEF
    rep stosb

    ; Calculate next row offset
    pop di
    pop cx

    ; Move to next row (need to handle interlacing)
    ; Current row in stack, need to track which row we're on
    ; For simplicity, just add 80 for even rows, handle odd rows
    push cx
    mov ax, 100
    add ax, 40
    sub ax, cx          ; Current row number

    test al, 1          ; Check if odd
    jz .even_row

    ; Odd row: next offset is +0x2000 - previous row offset + 80
    ; Actually, let's recalculate from scratch
    inc ax              ; Next row
    mov bx, ax          ; Save row number

    test al, 1
    jz .next_even

    ; Next is odd: 0x2000 + ((row-1)/2) * 80
    dec ax
    shr ax, 1
    mov dx, 80
    mul dx
    add ax, 0x2000
    mov di, ax
    jmp .continue

.next_even:
    ; Next is even: (row/2) * 80
    shr ax, 1
    mov dx, 80
    mul dx
    mov di, ax
    jmp .continue

.even_row:
    ; Current is even, next is odd
    inc ax
    mov bx, ax

    ; Odd: 0x2000 + ((row-1)/2) * 80
    dec ax
    shr ax, 1
    mov dx, 80
    mul dx
    add ax, 0x2000
    mov di, ax

.continue:
    pop cx
    loop .row_loop

    popa
    ret

; Data
composite_enabled db 0

; Messages (split across two lines for 40-column display)
msg_composite_off_1 db "  Mode: Normal CGA", 0
msg_composite_off_2 db "  Press key=Composite, ESC=Exit", 0
msg_composite_on_1 db "  Mode: Composite", 0
msg_composite_on_2 db "  Press key=Normal, ESC=Exit", 0
