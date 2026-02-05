; CGA Graphics Mode Test - Fixed
; Properly handles CGA interlaced memory

org 0x100

start:
    ; Switch to CGA mode 0x04
    mov ah, 0x00
    mov al, 0x04
    int 0x10

    ; Set palette 1 (cyan, magenta, white)
    mov dx, 0x3D9
    mov al, 0x30
    out dx, al

    ; Set up video segment
    mov ax, 0xB800
    mov es, ax

    ; Macro to draw a box at given row, column, with pattern
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
