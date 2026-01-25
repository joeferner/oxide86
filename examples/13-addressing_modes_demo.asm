; Comprehensive demonstration of 8086 memory addressing modes
; This program tests all implemented addressing modes

[BITS 16]
[ORG 0x0100]

start:
    ; ===========================================
    ; 1. Register Indirect: [BX], [SI], [DI], [BP]
    ; ===========================================
    mov bx, 0x0200
    mov si, 0x0210
    mov di, 0x0220

    mov byte [bx], 0x11     ; [BX]
    mov byte [si], 0x22     ; [SI]
    mov byte [di], 0x33     ; [DI]

    mov al, [bx]            ; AL = 0x11
    mov cl, [si]            ; CL = 0x22
    mov dl, [di]            ; DL = 0x33

    ; ===========================================
    ; 2. Based or Indexed with 8-bit Displacement
    ; ===========================================
    mov byte [bx+5], 0x44   ; [BX + disp8]
    mov byte [si+10], 0x55  ; [SI + disp8]
    mov byte [di+15], 0x66  ; [DI + disp8]

    mov ah, [bx+5]          ; AH = 0x44
    mov ch, [si+10]         ; CH = 0x55
    mov dh, [di+15]         ; DH = 0x66

    ; ===========================================
    ; 3. Based Indexed: [BX+SI], [BX+DI]
    ; ===========================================
    mov bx, 0x0100
    mov si, 0x0020
    mov di, 0x0030

    mov byte [bx+si], 0x77  ; [BX + SI]
    mov byte [bx+di], 0x88  ; [BX + DI]

    mov bl, [bx+si]         ; BL = 0x77
    mov bh, [bx+di]         ; BH = 0x88

    ; ===========================================
    ; 4. Based Indexed with Displacement
    ; ===========================================
    mov byte [bx+si+10], 0x99  ; [BX + SI + disp8]
    mov al, [bx+si+10]         ; AL = 0x99

    ; ===========================================
    ; 5. Direct Addressing: [offset]
    ; ===========================================
    mov byte [0x0400], 0xAA    ; Direct address
    mov al, [0x0400]           ; AL = 0xAA

    ; ===========================================
    ; 6. Arithmetic operations with memory
    ; ===========================================
    mov bx, 0x0500
    mov byte [bx], 0x10
    mov byte [bx+1], 0x20

    mov al, 0x05
    add al, [bx]            ; AL = 0x05 + 0x10 = 0x15

    mov cl, 0x30
    sub cl, [bx+1]          ; CL = 0x30 - 0x20 = 0x10

    mov dl, 0xFF
    and dl, [bx]            ; DL = 0xFF & 0x10 = 0x10

    ; ===========================================
    ; 7. 16-bit memory operations
    ; ===========================================
    mov word [bx+2], 0x1234
    mov ax, [bx+2]          ; AX = 0x1234

    mov word [bx+4], 0xABCD
    add ax, [bx+4]          ; AX = 0x1234 + 0xABCD = 0xBE01

    hlt
