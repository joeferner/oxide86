; EGA CRTC Start Address Test (mode 0x0D, 320x200x16)
;
; Fills two pages in EGA VRAM:
;   Page 0 (plane offset     0): solid white (color 15, all 4 planes = 0xFF)
;   Page 1 (plane offset  8000): solid blue  (color 1, plane 0 = 0xFF only)
;
; Verifies that writing CRTC registers 0x0C/0x0D (start address high/low)
; correctly offsets the display viewport to the new page.
;
; Screenshot 1: CRTC start = 0x0000 -> white screen (page 0 at offset 0)
; Screenshot 2: CRTC start = 0x1F40 -> blue  screen (page 1 at offset 8000)

[CPU 8086]
org 0x100

; Page 1 is at plane byte offset 8000 (200 rows * 40 bytes/row)
PAGE1_OFF    equ 8000
CRTC_PAGE1_HI equ 0x1F   ; high byte of 8000 = 0x1F40
CRTC_PAGE1_LO equ 0x40   ; low  byte of 8000 = 0x1F40

start:
    ; Set EGA mode 0x0D (320x200, 16 colors)
    mov ax, 0x000D
    int 0x10

    mov ax, 0xA000
    mov es, ax

    ; Enable all 4 planes
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    mov dx, 0x3C5
    mov al, 0x0F
    out dx, al

    ; --- Page 0: white (color 15, all planes 0xFF) ---
    xor di, di          ; offset 0
    mov cx, 8000
    mov al, 0xFF
    rep stosb

    ; --- Page 1: first clear all planes, then set plane 0 only ---
    ; Clear all planes at page 1 (map mask still 0x0F)
    mov di, PAGE1_OFF
    mov cx, 8000
    xor al, al
    rep stosb

    ; Select plane 0 only
    mov dx, 0x3C4
    mov al, 0x02
    out dx, al
    mov dx, 0x3C5
    mov al, 0x01
    out dx, al

    ; Write plane 0 with 0xFF -> color 1 (blue)
    mov di, PAGE1_OFF
    mov cx, 8000
    mov al, 0xFF
    rep stosb

    ; --- CRTC start = 0 (display page 0) ---
    mov dx, 0x3D4
    mov al, 0x0C        ; start address high
    out dx, al
    mov dx, 0x3D5
    mov al, 0x00
    out dx, al
    mov dx, 0x3D4
    mov al, 0x0D        ; start address low
    out dx, al
    mov dx, 0x3D5
    mov al, 0x00
    out dx, al

    ; Wait for keypress -> screenshot 1: white (page 0)
    mov ah, 0x00
    int 0x16

    ; --- CRTC start = 0x1F40 (display page 1) ---
    mov dx, 0x3D4
    mov al, 0x0C
    out dx, al
    mov dx, 0x3D5
    mov al, CRTC_PAGE1_HI
    out dx, al
    mov dx, 0x3D4
    mov al, 0x0D
    out dx, al
    mov dx, 0x3D5
    mov al, CRTC_PAGE1_LO
    out dx, al

    ; Wait for keypress -> screenshot 2: blue (page 1)
    mov ah, 0x00
    int 0x16

    ; Return to text mode and exit
    mov ax, 0x0003
    int 0x10
    mov ah, 0x4C
    xor al, al
    int 0x21
