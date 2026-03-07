; CGA Graphics Mode 0x06 Test
; 640x200, 2 Colors (1 bit per pixel)
; Properly handles CGA interlaced memory

[CPU 8086]
org 0x100

start:
    ; Switch to CGA mode 0x06 (640x200, 2 colors)
    mov ah, 0x00
    mov al, 0x06
    int 0x10

    ; Set up video segment for direct memory access
    mov ax, 0xB800
    mov es, ax

    ; Draw boxes FIRST (so text can overlay them)
    ; Box 1: Row 0, Col 0, Solid (0xFF = 8 pixels on per byte)
    mov word [box_row], 0
    mov word [box_col], 0
    mov byte [box_pattern], 0xFF
    call draw_box

    ; Box 2: Row 0, Col 20 (pixel 160), Dense checkerboard
    mov word [box_row], 0
    mov word [box_col], 20
    mov byte [box_pattern], 0xCC
    call draw_box

    ; Box 3: Row 0, Col 40 (pixel 320), Alternating
    mov word [box_row], 0
    mov word [box_col], 40
    mov byte [box_pattern], 0x55
    call draw_box

    ; Box 4: Row 100, Col 0, Alternating (0xAA)
    mov word [box_row], 100
    mov word [box_col], 0
    mov byte [box_pattern], 0xAA
    call draw_box

    ; Box 5: Row 100, Col 20, Sparse pattern
    mov word [box_row], 100
    mov word [box_col], 20
    mov byte [box_pattern], 0x66
    call draw_box

    ; Box 6: Row 100, Col 40, Sparse checkerboard
    mov word [box_row], 100
    mov word [box_col], 40
    mov byte [box_pattern], 0x99
    call draw_box

    ; Print text in the middle area (rows 40-99, chars 5-12)
    ; In mode 6 the character grid is 80x25 (640/8=80 cols, 200/8=25 rows)

    ; Position cursor at row 6, col 10
    mov ah, 0x02
    mov bh, 0
    mov dh, 6           ; Row 6 (48 pixels from top)
    mov dl, 10          ; Column 10
    int 0x10

    ; Print header text using BIOS teletype
    mov si, msg_header
    mov bl, 1           ; Foreground color
    call print_string

    ; Position cursor for next line (row 8)
    mov ah, 0x02
    mov bh, 0
    mov dh, 8
    mov dl, 10
    int 0x10

    ; Print info
    mov si, msg_info
    mov bl, 1
    call print_string

    ; Print completion message at bottom
    mov ah, 0x02
    mov bh, 0
    mov dh, 23          ; Row 23
    mov dl, 5           ; Column 5
    int 0x10

    mov si, msg_done
    mov bl, 1
    call print_string

    ; Wait for keypress
    mov ah, 0x00
    int 0x16

    ; Return to text mode
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    ; Exit
    mov ah, 0x4C    ; DOS terminate with return code
    mov al, 0x00    ; exit code 0
    int 0x21        ; In DOS: exits. In emulator: halts.

; Print null-terminated string using BIOS teletype
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
    mov ah, 0x0E        ; Teletype output
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

; Draw a 10-byte wide, 40-row tall box
; Parameters: box_row, box_col (byte offset 0-79), box_pattern
; Each byte = 8 pixels; 10 bytes = 80 pixels wide
draw_box:
    push ax
    push bx
    push cx
    push dx
    push si
    push di

    mov cx, 40              ; 40 rows
    mov si, [box_row]       ; Starting row

.row_loop:
    push cx

    ; Calculate CGA interlaced offset for current row
    mov ax, si              ; Current row
    test al, 1              ; Check if odd
    jz .even_row

    ; Odd row: 0x2000 + ((row-1)/2) * 80 + col
    dec ax
    shr ax, 1
    mov bx, 80
    mul bx
    add ax, 0x2000
    add ax, [box_col]
    mov di, ax
    jmp .write_row

.even_row:
    ; Even row: (row/2) * 80 + col
    shr ax, 1
    mov bx, 80
    mul bx
    add ax, [box_col]
    mov di, ax

.write_row:
    ; Write 10 bytes (10 * 8 = 80 pixels wide)
    mov cx, 10
    mov al, [box_pattern]
.write_loop:
    mov [es:di], al
    inc di
    loop .write_loop

    inc si                  ; Next row
    pop cx
    loop .row_loop

    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Data
box_row dw 0
box_col dw 0
box_pattern db 0

; Messages
msg_header db "CGA Graphics Mode 0x06 Test", 13, 10, 0
msg_info db "640x200, 2 Colors (1 bit/pixel)", 13, 10, 0
msg_done db "Test complete! Press any key...", 0
