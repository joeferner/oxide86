; Write 'H' at the top-left corner (0,0)

[CPU 8086]
org 0x100

start:
    ; 1. Point ES to the Color Text Video Segment (0xB800)
    mov ax, 0xB800
    mov es, ax

    ; 2. Define our character and attribute
    ; ASCII 'H' = 48h
    ; Attribute 0Fh = Bright White (F) on Black (0)
    ; In memory, it's stored as [Character][Attribute] (Little Endian in AX)
    
    mov ax, 0x0F48      ; AH = 0F (Attr), AL = 48 (Char)
    mov [es:0], ax      ; Write 'H' to the very first character cell

    hlt
