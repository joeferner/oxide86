; boot_test.asm - Simple boot sector test
; This is loaded at 0x0000:0x7C00 by the BIOS
; It prints a message and halts

[BITS 16]       ; 16-bit code
[ORG 0x7C00]    ; Boot sector is loaded at 0x7C00

start:
    ; Clear direction flag
    cld

    ; Print boot message using INT 10h
    mov si, boot_msg
    call print_string

    ; Halt the CPU
    hlt

;------------------------------------------------------------
; print_string - Print null-terminated string
; Input: SI = pointer to string
;------------------------------------------------------------
print_string:
    push ax
    push bx
.loop:
    lodsb               ; Load byte from [DS:SI] into AL, increment SI
    test al, al         ; Check if zero (end of string)
    jz .done
    mov ah, 0x0E        ; INT 10h, AH=0Eh - Teletype output
    mov bh, 0           ; Page number
    int 0x10
    jmp .loop
.done:
    pop bx
    pop ax
    ret

;------------------------------------------------------------
; Data
;------------------------------------------------------------
boot_msg db 'Boot sector loaded successfully!', 13, 10
         db 'BIOS has transferred control to 0x0000:0x7C00', 13, 10
         db 'This is the start of the boot process.', 13, 10, 0

;------------------------------------------------------------
; Boot sector signature
; Must be at offset 510-511 (0x55AA in little-endian)
;------------------------------------------------------------
times 510-($-$$) db 0   ; Pad to 510 bytes
dw 0xAA55               ; Boot signature (little-endian: 0x55 0xAA)
