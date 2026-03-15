; Video Hardware Detection Test
; Extracted from CT.ASM (CT755r CGA test suite) by Sergey Kiselev
; Detects CGA, EGA, and MDA/HGC cards and reports which is active.
;
; Subroutines find6845, findmono, and print_str are inlined from SUBR.ASM.

[CPU 8086]
org 0x100
BITS 16

start:
    ; Set text mode 3 (80x25 color)
    xor ah, ah
    mov al, 3
    int 0x10

    ;--------------------detect CGA 6845 compatible card------------------------
    mov si, detect_cga_str
    call print_str
    mov dx, 0x3D4           ; CGA CRTC address port
    call find6845
    mov si, none_str
    jc .cga_end
    mov si, ok_str
.cga_end:
    call print_str

    ;--------------------------detect EGA card------------------------------
    mov si, detect_ega_str
    call print_str
    mov ax, 0x40
    mov es, ax
    mov al, [es:0x87]
    mov si, none_str
    cmp al, 0
    jz .ega_det_end         ; al == 0 means no EGA, print NONE
    test al, 0x08           ; bit3=1 means EGA present but inactive
    jnz .ega_size
    mov byte [act_card], 0x01   ; bit3=0, EGA is active card
.ega_size:
    mov ah, 0x12
    mov bl, 0x10
    int 0x10
    and bl, 0x03
    mov si, m64k_str
    cmp bl, 0x00
    je .ega_det_end
    mov si, m128k_str
    cmp bl, 0x01
    je .ega_det_end
    mov si, m192k_str
    cmp bl, 0x02
    je .ega_det_end
    mov si, m256k_str
.ega_det_end:
    call print_str

    ;--------------------------detect MDA/HGC card------------------------------
    mov si, detect_mono_str
    call print_str
    call findmono           ; CF=1 if no MDA/HGC; AL=01h if MDA, else HGC
    mov si, none_str
    jc .mono_end
    mov si, mda_str
    cmp al, 0x01
    je .mono_end
    mov si, hgc_str
.mono_end:
    call print_str

    ;--------------------------detect active card------------------------------
    mov si, active_card_str
    call print_str
    mov si, none_str
    cmp byte [act_card], 0x01   ; EGA was flagged as active?
    jne .check_bda
    mov si, ega_str
    call print_str
    jmp .wait_key

.check_bda:
    mov ax, 0x40
    mov es, ax
    mov al, [es:0x10]
    and al, 0x30            ; bits 4-5 encode initial video hardware
    cmp al, 0x30            ; 11 = MONO
    jne .not_mono
    mov si, mono_str
    call print_str
    jmp .wait_key

.not_mono:
    cmp al, 0x10            ; 01 = CGA 40x25
    jne .not_cga40
    mov si, cga40_str
    call print_str
    jmp .wait_key

.not_cga40:
    cmp al, 0x20            ; 10 = CGA 80x25
    jne .print_none
    mov si, cga80_str
    call print_str
    jmp .wait_key

.print_none:
    call print_str          ; print none_str

.wait_key:
    xor ah, ah
    int 0x16

    ; Return to text mode
    xor ah, ah
    mov al, 3
    int 0x10

    mov ah, 0x4C
    xor al, al
    int 0x21

;----------------------------find6845------------------------------------------
; Tests for a 6845-compatible CRTC chip at port DX.
; Input:  DX = CRTC address port (0x3D4 for CGA, 0x3B4 for MDA)
; Output: CF clear if 6845 found, CF set if not found
find6845:
    mov al, 0x0F            ; cursor low register
    out dx, al
    inc dx                  ; switch to data port
    in al, dx
    mov ah, al              ; save original value
    mov al, 0x55            ; probe value
    out dx, al
    mov cx, 0x100           ; wait for chip to react
.f6845_wait:
    loop .f6845_wait
    in al, dx
    xchg ah, al
    out dx, al              ; restore original value
    cmp ah, 0x55
    je .f6845_found
    stc                     ; not found
.f6845_found:
    ret

;----------------------------findmono------------------------------------------
; Detects MDA or HGC adapter at 0x3B4.
; Output: CF set if none found
;         CF clear, AL=01h if MDA
;         CF clear, AL=00h if HGC (or variant)
findmono:
    mov dx, 0x3B4
    call find6845
    jc .fm_exit
    mov dx, 0x3BA           ; status port
    in al, dx
    and al, 0x80
    mov ah, al
    mov cx, 0x8000
.fm_wait:
    in al, dx
    and al, 0x80
    cmp ah, al
    loope .fm_wait          ; loop while bit7 unchanged
    jne .fm_hgc             ; bit7 changed -> HGC
    mov al, 0x01            ; no change -> MDA
    jmp .fm_exit
.fm_hgc:
    in al, dx
    and al, 0x70
.fm_exit:
    ret

;----------------------------print_str-----------------------------------------
; Prints a null-terminated string via INT 10h teletype output.
; Input: SI = pointer to string
print_str:
    push ax
    push bx
.ps_loop:
    lodsb
    cmp al, 0
    je .ps_done
    mov bx, 0x0001          ; page 0, color 1
    mov ah, 0x0E
    int 0x10
    jmp .ps_loop
.ps_done:
    pop bx
    pop ax
    ret

;----------------------------data----------------------------------------------
act_card        db 0x00

detect_cga_str  db " Detect CGA 6845 card...", 0
detect_ega_str  db " Detect EGA card........", 0
detect_mono_str db " Detect MONO card.......", 0
active_card_str db " Active card............", 0

none_str    db "NONE", 13, 10, 0
ok_str      db "OK", 13, 10, 0

mono_str    db "MONO", 13, 10, 0
mda_str     db "MDA",  13, 10, 0
hgc_str     db "HGC",  13, 10, 0
ega_str     db "EGA",  13, 10, 0
cga40_str   db "CGA 40x25", 13, 10, 0
cga80_str   db "CGA 80x25", 13, 10, 0

m64k_str    db "OK, 064 KB RAM", 13, 10, 0
m128k_str   db "OK, 128 KB RAM", 13, 10, 0
m192k_str   db "OK, 192 KB RAM", 13, 10, 0
m256k_str   db "OK, 256 KB RAM", 13, 10, 0
