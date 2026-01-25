; BIOS Data Area (BDA) Test Program
; This program reads and displays values from the BIOS Data Area at 0x0040:0000

org 0x100

section .text
start:
    ; Set up BDA segment in ES
    mov ax, 0x0040
    mov es, ax

    ; Display header
    mov dx, msg_header
    mov ah, 0x09
    int 0x21

    ; Read and display equipment list (0x0040:0010)
    mov dx, msg_equipment
    mov ah, 0x09
    int 0x21

    mov ax, [es:0x10]   ; Read equipment word
    call print_hex_word
    call print_newline

    ; Read and display memory size (0x0040:0013)
    mov dx, msg_memory
    mov ah, 0x09
    int 0x21

    mov ax, [es:0x13]   ; Read memory size in KB
    call print_hex_word
    call print_newline

    ; Read and display video mode (0x0040:0049)
    mov dx, msg_video_mode
    mov ah, 0x09
    int 0x21

    mov al, [es:0x49]   ; Read video mode
    call print_hex_byte
    call print_newline

    ; Read and display screen columns (0x0040:004A)
    mov dx, msg_columns
    mov ah, 0x09
    int 0x21

    mov ax, [es:0x4A]   ; Read screen columns
    call print_hex_word
    call print_newline

    ; Read and display CRTC port (0x0040:0063)
    mov dx, msg_crtc_port
    mov ah, 0x09
    int 0x21

    mov ax, [es:0x63]   ; Read CRTC port address
    call print_hex_word
    call print_newline

    ; Read and display COM1 port (0x0040:0000)
    mov dx, msg_com1
    mov ah, 0x09
    int 0x21

    mov ax, [es:0x00]   ; Read COM1 port address
    call print_hex_word
    call print_newline

    ; Read and display LPT1 port (0x0040:0008)
    mov dx, msg_lpt1
    mov ah, 0x09
    int 0x21

    mov ax, [es:0x08]   ; Read LPT1 port address
    call print_hex_word
    call print_newline

    ; Exit
    mov dx, msg_done
    mov ah, 0x09
    int 0x21

    mov ax, 0x4C00
    int 0x21

; Print a 16-bit value in hex
; Input: AX = value to print
print_hex_word:
    push ax
    mov al, ah          ; Print high byte first
    call print_hex_byte
    pop ax
    call print_hex_byte ; Print low byte
    ret

; Print an 8-bit value in hex
; Input: AL = value to print
print_hex_byte:
    push ax
    shr al, 4           ; Get high nibble
    call print_hex_nibble
    pop ax
    and al, 0x0F        ; Get low nibble
    call print_hex_nibble
    ret

; Print a single hex digit (0-F)
; Input: AL = nibble (0-15)
print_hex_nibble:
    push ax
    push dx
    cmp al, 10
    jl .digit
    add al, 'A' - 10
    jmp .print
.digit:
    add al, '0'
.print:
    mov dl, al
    mov ah, 0x02
    int 0x21
    pop dx
    pop ax
    ret

; Print newline (CR+LF)
print_newline:
    push ax
    push dx
    mov dl, 0x0D
    mov ah, 0x02
    int 0x21
    mov dl, 0x0A
    mov ah, 0x02
    int 0x21
    pop dx
    pop ax
    ret

section .data
msg_header:     db '=== BIOS Data Area Test ===', 0x0D, 0x0A, '$'
msg_equipment:  db 'Equipment List: 0x$'
msg_memory:     db 'Memory Size KB: 0x$'
msg_video_mode: db 'Video Mode:     0x$'
msg_columns:    db 'Screen Columns: 0x$'
msg_crtc_port:  db 'CRTC Port:      0x$'
msg_com1:       db 'COM1 Port:      0x$'
msg_lpt1:       db 'LPT1 Port:      0x$'
msg_done:       db 'Test complete!', 0x0D, 0x0A, '$'
