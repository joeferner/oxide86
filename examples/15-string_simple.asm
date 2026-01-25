; Simple String Instructions Test
; Tests basic STOS, MOVS, and LODS operations

[BITS 16]
[ORG 0x0000]

start:
    ; Setup segments
    mov ax, 0x1000
    mov ds, ax
    mov es, ax

    ;===========================================
    ; Test STOSB - Fill memory with pattern
    ;===========================================
    cld                 ; Forward direction
    mov di, 0x0100      ; Destination
    mov al, 0x42        ; 'B'
    stosb               ; Store byte, DI becomes 0x0101
    mov al, 0x43        ; 'C'
    stosb               ; Store byte, DI becomes 0x0102
    mov al, 0x44        ; 'D'
    stosb               ; Store byte, DI becomes 0x0103
    ; Memory at 0x0100: 42 43 44

    ;===========================================
    ; Test LODSB - Load from memory
    ;===========================================
    mov si, 0x0100      ; Source
    lodsb               ; AL = 0x42, SI becomes 0x0101
    mov bx, ax          ; Save first byte in BX
    lodsb               ; AL = 0x43, SI becomes 0x0102
    lodsb               ; AL = 0x44, SI becomes 0x0103
    ; AL = 0x44, BX = 0x0042

    ;===========================================
    ; Test MOVSB - Copy memory
    ;===========================================
    mov si, 0x0100      ; Source
    mov di, 0x0200      ; Destination
    movsb               ; Copy byte, both increment
    movsb               ; Copy byte
    movsb               ; Copy byte
    ; Memory at 0x0200: 42 43 44

    hlt
