; INT 10h Function 07h - Scroll Down
; Fill col 0 of each row with a distinct char: rows 0-9 -> '0'-'9', rows 10-24 -> 'A'-'O'
; Scroll down 1 line across the full screen, then verify:
;   row  0 = ' ' (new blank line inserted at top)
;   row  1 = '0' (was at row 0)
;   row 10 = '9' (was at row 9)
;   row 11 = 'A' (was at row 10)
;   row 24 = 'N' (was at row 23: 'A'+13)

[CPU 8086]
org 0x0100

start:
    ; Set video mode 3 for clean 80x25 text screen
    mov ah, 0x00
    mov al, 0x03
    int 0x10

    ; Fill col 0 of rows 0-24 with distinct characters
    xor di, di
fill_loop:
    ; Set cursor to (row=DI, col=0)
    mov ah, 0x02
    mov bh, 0
    mov dx, di          ; DH=0, DL=row
    xchg dh, dl         ; DH=row, DL=0
    int 0x10

    ; Compute character: DI < 10 -> '0'+DI, else 'A'+(DI-10)
    mov ax, di
    cmp al, 10
    jl .digit
    sub al, 10
    add al, 'A'
    jmp .write
.digit:
    add al, '0'
.write:
    mov ah, 0x09
    mov bh, 0
    mov bl, 0x07
    mov cx, 1
    int 0x10

    inc di
    cmp di, 25
    jl fill_loop

    ; Scroll down 1 line across full screen
    mov ah, 0x07
    mov al, 1           ; scroll 1 line
    mov bh, 0x07        ; blank line attribute
    mov ch, 0           ; top row
    mov cl, 0           ; left col
    mov dh, 24          ; bottom row
    mov dl, 79          ; right col
    int 0x10

    ; Verify row 0 = ' ' (blank line inserted at top)
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 0
    int 0x10
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, ' '
    jne fail

    ; Verify row 1 = '0' (was at row 0)
    mov ah, 0x02
    mov bh, 0
    mov dh, 1
    mov dl, 0
    int 0x10
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, '0'
    jne fail

    ; Verify row 10 = '9' (was at row 9)
    mov ah, 0x02
    mov bh, 0
    mov dh, 10
    mov dl, 0
    int 0x10
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, '9'
    jne fail

    ; Verify row 11 = 'A' (was at row 10)
    mov ah, 0x02
    mov bh, 0
    mov dh, 11
    mov dl, 0
    int 0x10
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'A'
    jne fail

    ; Verify row 24 = 'N' (was at row 23: 'A'+13)
    mov ah, 0x02
    mov bh, 0
    mov dh, 24
    mov dl, 0
    int 0x10
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'N'
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
