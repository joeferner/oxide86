; CGA Graphics Mode Test - Fixed
; Properly handles CGA interlaced memory

org 0x100

start:
    ; Switch to CGA mode 0x04 (320x200, 4 colors)
    mov ah, 0x00
    mov al, 0x04
    int 0x10

    ; Set palette 1 (cyan, magenta, white) with intensity
    mov dx, 0x3D9
    mov al, 0x30        ; Palette 1, intensity on
    out dx, al

    ; Set up video segment for direct memory access
    mov ax, 0xB800
    mov es, ax

    ; Draw boxes FIRST (so text can overlay them)
    ; Box 1: Row 0, Col 0, Cyan
    mov word [box_row], 0
    mov word [box_col], 0
    mov byte [box_pattern], 0x55
    call draw_box

    ; Box 2: Row 0, Col 20, Magenta
    mov word [box_row], 0
    mov word [box_col], 20
    mov byte [box_pattern], 0xAA
    call draw_box

    ; Box 3: Row 0, Col 40, White
    mov word [box_row], 0
    mov word [box_col], 40
    mov byte [box_pattern], 0xFF
    call draw_box

    ; Box 4: Row 100, Col 0, Cyan
    mov word [box_row], 100
    mov word [box_col], 0
    mov byte [box_pattern], 0x55
    call draw_box

    ; Box 5: Row 100, Col 20, Pattern
    mov word [box_row], 100
    mov word [box_col], 20
    mov byte [box_pattern], 0xE4
    call draw_box

    ; Box 6: Row 100, Col 40, Pattern
    mov word [box_row], 100
    mov word [box_col], 40
    mov byte [box_pattern], 0xE4
    call draw_box

    ; Now print text in the middle (between top and bottom boxes)
    ; Top boxes end at row 39, bottom boxes start at row 100
    ; Middle area is rows 40-99 (character rows 5-12)

    ; Position cursor at row 6 (pixel row 48), centered horizontally
    mov ah, 0x02
    mov bh, 0
    mov dh, 6           ; Row 6 (48 pixels from top)
    mov dl, 6           ; Column 6 (48 pixels from left)
    int 0x10

    ; Print header text using BIOS teletype
    mov si, msg_header
    mov bl, 3           ; White color
    call print_string

    ; Position cursor for next line (row 8)
    mov ah, 0x02
    mov bh, 0
    mov dh, 8           ; Row 8
    mov dl, 3           ; Column 3
    int 0x10

    ; Print instructions
    mov si, msg_info
    mov bl, 2           ; Magenta color
    call print_string

    ; Print completion message at bottom
    mov ah, 0x02
    mov bh, 0
    mov dh, 23          ; Row 23
    mov dl, 2           ; Column 2
    int 0x10

    mov si, msg_done
    mov bl, 1           ; Cyan color
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
    int 0x21

; Print null-terminated string using BIOS teletype
; Input: SI = pointer to string, BL = color
print_string:
    pusha
.loop:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0E        ; Teletype output
    int 0x10
    jmp .loop
.done:
    popa
    ret

; Draw a 10-byte wide, 40-row tall box
; Parameters: box_row, box_col, box_pattern
draw_box:
    pusha
    
    mov cx, 40              ; 40 rows
    mov si, [box_row]       ; Starting row
    
.row_loop:
    push cx
    
    ; Calculate CGA offset for current row
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
    ; Write 10 bytes
    mov cx, 10
    mov al, [box_pattern]
.write_loop:
    mov [es:di], al
    inc di
    loop .write_loop
    
    inc si                  ; Next row
    pop cx
    loop .row_loop
    
    popa
    ret

; Data
box_row dw 0
box_col dw 0
box_pattern db 0

; Messages
msg_header db "CGA Graphics Mode 0x04 Test", 13, 10, 0
msg_info db "Drawing test patterns...", 13, 10, 0
msg_done db "Test complete! Press any key...", 0
