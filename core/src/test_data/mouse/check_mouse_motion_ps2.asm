; PS/2 mouse motion test: initialize the PS/2 mouse via INT 15h AH=C2h,
; register a callback handler, wait for one motion packet and verify
; dx=10, dy=5, no buttons pressed.
;
; PS/2 mouse BIOS callback convention (INT 15h AH=C2h AL=07h):
;   AL = status byte: bit 0=left, bit 1=right, bit 4=X sign, bit 5=Y sign
;   BL = X delta (signed byte)
;   CL = Y delta (signed byte)
;   DL = Z delta (0 for standard mouse)
; Must return with RETF.
;
; For dx=10, dy=5, buttons=0:
;   byte0 = 0x08 (always-1 bit, no buttons, no sign bits)
;   byte1 = 0x0A (dx=10)
;   byte2 = 0x05 (dy=5)
; Callback receives: AL=0x08, BL=0x0A, CL=0x05

[CPU 8086]
org 0x0100

start:
    ; Initialize PS/2 mouse: 3-byte packets (AL=05h, BH=03h)
    mov ax, 0xC205
    mov bh, 3
    int 0x15
    jc fail

    ; Enable PS/2 mouse (AL=00h, BH=01h)
    mov ax, 0xC200
    mov bh, 1
    int 0x15
    jc fail

    ; Register callback handler (AL=07h, ES:BX = far pointer)
    mov ax, 0xC207
    push cs
    pop es
    mov bx, mouse_handler
    int 0x15
    jc fail

poll:
    cmp byte [mouse_ready], 0
    je poll

    ; Verify no buttons (status bits 0 and 1 must be clear)
    mov al, [mouse_status]
    and al, 0x03
    jnz fail

    ; Verify dx=10
    cmp byte [mouse_dx], 0x0A
    jne fail

    ; Verify dy=5
    cmp byte [mouse_dy], 0x05
    jne fail

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

; -----------------------------------------------------------------------
; mouse_handler: PS/2 mouse callback (FAR CALL from INT 74h BIOS handler)
; -----------------------------------------------------------------------
mouse_handler:
    mov [mouse_status], al
    mov [mouse_dx], bl
    mov [mouse_dy], cl
    mov byte [mouse_ready], 1
    retf

mouse_status: db 0
mouse_dx:     db 0
mouse_dy:     db 0
mouse_ready:  db 0
