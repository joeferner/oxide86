; INT 10h Function 0Bh - Set Color Palette
; BH=0: set border/background color (BL = color 0-15)
; BH=1: select CGA foreground palette (BL=0 or BL=1)
;
; What you should see on screen:
;   Row 0: "CMW" - cyan (03), magenta (05), white (07) written while palette 1 active
;   Row 1: "GRY" - green (02), red (04), yellow (06) written while palette 0 active
;
; Verifies palette selection does not corrupt the video mode (mode 3), and that
; characters written with palette-matching attributes round-trip correctly.

[CPU 8086]
org 0x0100

start:
    ; Select CGA palette 1 (cyan/magenta/white): BH=1, BL=1
    mov ah, 0x0B
    mov bh, 1
    mov bl, 1
    int 0x10

    ; Video mode should still be 3
    mov ah, 0x0F
    int 0x10
    cmp al, 0x03
    jne fail

    ; Write 'C' in cyan (attr 0x03) at row 0, col 0
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 0
    int 0x10
    mov ah, 0x09
    mov al, 'C'
    mov bh, 0
    mov bl, 0x03        ; cyan
    mov cx, 1
    int 0x10

    ; Read back and verify
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'C'
    jne fail
    cmp ah, 0x03
    jne fail

    ; Write 'M' in magenta (attr 0x05) at row 0, col 1
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 1
    int 0x10
    mov ah, 0x09
    mov al, 'M'
    mov bh, 0
    mov bl, 0x05        ; magenta
    mov cx, 1
    int 0x10

    ; Read back and verify
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'M'
    jne fail
    cmp ah, 0x05
    jne fail

    ; Write 'W' in white (attr 0x07) at row 0, col 2
    mov ah, 0x02
    mov bh, 0
    mov dh, 0
    mov dl, 2
    int 0x10
    mov ah, 0x09
    mov al, 'W'
    mov bh, 0
    mov bl, 0x07        ; white
    mov cx, 1
    int 0x10

    ; Read back and verify
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'W'
    jne fail
    cmp ah, 0x07
    jne fail

    ; Select CGA palette 0 (green/red/yellow): BH=1, BL=0
    mov ah, 0x0B
    mov bh, 1
    mov bl, 0
    int 0x10

    ; Video mode should still be 3
    mov ah, 0x0F
    int 0x10
    cmp al, 0x03
    jne fail

    ; Write 'G' in green (attr 0x02) at row 1, col 0
    mov ah, 0x02
    mov bh, 0
    mov dh, 1
    mov dl, 0
    int 0x10
    mov ah, 0x09
    mov al, 'G'
    mov bh, 0
    mov bl, 0x02        ; green
    mov cx, 1
    int 0x10

    ; Read back and verify
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'G'
    jne fail
    cmp ah, 0x02
    jne fail

    ; Write 'R' in red (attr 0x04) at row 1, col 1
    mov ah, 0x02
    mov bh, 0
    mov dh, 1
    mov dl, 1
    int 0x10
    mov ah, 0x09
    mov al, 'R'
    mov bh, 0
    mov bl, 0x04        ; red
    mov cx, 1
    int 0x10

    ; Read back and verify
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'R'
    jne fail
    cmp ah, 0x04
    jne fail

    ; Write 'Y' in yellow (attr 0x06) at row 1, col 2
    mov ah, 0x02
    mov bh, 0
    mov dh, 1
    mov dl, 2
    int 0x10
    mov ah, 0x09
    mov al, 'Y'
    mov bh, 0
    mov bl, 0x06        ; yellow
    mov cx, 1
    int 0x10

    ; Read back and verify
    mov ah, 0x08
    mov bh, 0
    int 0x10
    cmp al, 'Y'
    jne fail
    cmp ah, 0x06
    jne fail

    ; Set border color to green (BH=0, BL=2) and verify mode intact
    mov ah, 0x0B
    mov bh, 0
    mov bl, 2
    int 0x10

    mov ah, 0x0F
    int 0x10
    cmp al, 0x03
    jne fail

    ; Reset border color to black
    mov ah, 0x0B
    mov bh, 0
    mov bl, 0
    int 0x10

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
