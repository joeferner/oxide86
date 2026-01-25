; Test INT 13h BIOS Disk Services
; This program demonstrates reading and writing sectors using CHS addressing

bits 16
org 0x0100

start:
    ; Print startup message
    mov dx, msg_start
    mov ah, 0x09
    int 0x21

    ; Test 1: Get Drive Parameters (INT 13h, AH=08h)
    call test_get_params

    ; Test 2: Reset Disk System (INT 13h, AH=00h)
    call test_reset

    ; Test 3: Write Sectors (INT 13h, AH=03h)
    call test_write

    ; Test 4: Read Sectors (INT 13h, AH=02h)
    call test_read

    ; Exit program
    mov ax, 0x4C00
    int 0x21

;------------------------------------------------------------------------------
; Test INT 13h, AH=08h - Get Drive Parameters
;------------------------------------------------------------------------------
test_get_params:
    mov dx, msg_get_params
    mov ah, 0x09
    int 0x21

    mov dl, 0x00            ; Drive 0 (floppy A:)
    mov ah, 0x08            ; Get drive parameters
    int 0x13

    jc .error

    ; Success - display parameters
    mov dx, msg_success
    mov ah, 0x09
    int 0x21

    ; Display max cylinder (CH)
    mov al, ch
    call print_hex_byte

    ; Display max head (DH)
    mov al, dh
    call print_hex_byte

    ; Display max sector (CL bits 0-5)
    mov al, cl
    and al, 0x3F
    call print_hex_byte

    mov dx, msg_newline
    mov ah, 0x09
    int 0x21

    ret

.error:
    mov dx, msg_error
    mov ah, 0x09
    int 0x21
    ret

;------------------------------------------------------------------------------
; Test INT 13h, AH=00h - Reset Disk System
;------------------------------------------------------------------------------
test_reset:
    mov dx, msg_reset
    mov ah, 0x09
    int 0x21

    mov dl, 0x00            ; Drive 0
    mov ah, 0x00            ; Reset disk
    int 0x13

    jc .error

    mov dx, msg_success
    mov ah, 0x09
    int 0x21
    ret

.error:
    mov dx, msg_error
    mov ah, 0x09
    int 0x21
    ret

;------------------------------------------------------------------------------
; Test INT 13h, AH=03h - Write Sectors
;------------------------------------------------------------------------------
test_write:
    mov dx, msg_write
    mov ah, 0x09
    int 0x21

    ; Prepare test data
    mov si, test_data
    mov di, buffer
    mov cx, 512
.copy_loop:
    lodsb
    stosb
    loop .copy_loop

    ; Write 1 sector to C=0, H=0, S=1
    mov al, 0x01            ; Number of sectors
    mov ch, 0x00            ; Cylinder 0
    mov cl, 0x01            ; Sector 1
    mov dh, 0x00            ; Head 0
    mov dl, 0x00            ; Drive 0
    mov bx, buffer          ; ES:BX = buffer address
    mov ah, 0x03            ; Write sectors
    int 0x13

    jc .error

    mov dx, msg_success
    mov ah, 0x09
    int 0x21
    ret

.error:
    mov dx, msg_error
    mov ah, 0x09
    int 0x21
    ret

;------------------------------------------------------------------------------
; Test INT 13h, AH=02h - Read Sectors
;------------------------------------------------------------------------------
test_read:
    mov dx, msg_read
    mov ah, 0x09
    int 0x21

    ; Clear buffer first
    mov di, buffer
    mov cx, 512
    xor al, al
.clear_loop:
    stosb
    loop .clear_loop

    ; Read 1 sector from C=0, H=0, S=1
    mov al, 0x01            ; Number of sectors
    mov ch, 0x00            ; Cylinder 0
    mov cl, 0x01            ; Sector 1
    mov dh, 0x00            ; Head 0
    mov dl, 0x00            ; Drive 0
    mov bx, buffer          ; ES:BX = buffer address
    mov ah, 0x02            ; Read sectors
    int 0x13

    jc .error

    mov dx, msg_success
    mov ah, 0x09
    int 0x21

    ; Verify first few bytes match test_data
    mov si, test_data
    mov di, buffer
    mov cx, 16
.verify:
    lodsb
    scasb
    jne .verify_fail
    loop .verify

    mov dx, msg_verify_ok
    mov ah, 0x09
    int 0x21
    ret

.verify_fail:
    mov dx, msg_verify_fail
    mov ah, 0x09
    int 0x21
    ret

.error:
    mov dx, msg_error
    mov ah, 0x09
    int 0x21
    ret

;------------------------------------------------------------------------------
; Print AL as hex byte
;------------------------------------------------------------------------------
print_hex_byte:
    push ax
    push dx

    mov ah, al
    shr al, 4
    call print_hex_digit

    mov al, ah
    and al, 0x0F
    call print_hex_digit

    pop dx
    pop ax
    ret

print_hex_digit:
    cmp al, 10
    jb .decimal
    add al, 'A' - 10
    jmp .print
.decimal:
    add al, '0'
.print:
    mov dl, al
    mov ah, 0x02
    int 0x21
    ret

;------------------------------------------------------------------------------
; Data
;------------------------------------------------------------------------------
msg_start:          db 'INT 13h Disk Services Test', 13, 10, '$'
msg_get_params:     db 'Testing Get Drive Parameters... $'
msg_reset:          db 'Testing Reset Disk... $'
msg_write:          db 'Testing Write Sectors... $'
msg_read:           db 'Testing Read Sectors... $'
msg_success:        db 'OK ', '$'
msg_error:          db 'ERROR!', 13, 10, '$'
msg_verify_ok:      db 'Data verified!', 13, 10, '$'
msg_verify_fail:    db 'Verification failed!', 13, 10, '$'
msg_newline:        db 13, 10, '$'

test_data:
    db 'HELLO DISK TEST!', 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07
    times 496 db 0xAA   ; Fill rest with pattern

buffer:
    times 512 db 0
