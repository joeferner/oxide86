; ATA IDENTIFY DEVICE test
; Selects primary master, issues IDENTIFY (0xECh), reads model string from word 27
; Run: cargo run -p oxide86-native-cli -- test-programs/cdrom/ata_identify.com
;
; Expected output: "Oxide86 ATA Hard Drive" (if HDD attached)
; or "ATAPI" in cylinder regs (if CD-ROM on master)

org 0x100

start:
    ; Select master device (DEV=0)
    mov dx, 0x1F6
    mov al, 0xA0
    out dx, al

    ; Wait ~400ns by reading alt-status 4 times (common practice)
    mov dx, 0x3F6
    in al, dx
    in al, dx
    in al, dx
    in al, dx

.wait_ready:
    mov dx, 0x1F7       ; Status register
    in al, dx
    test al, 0x80       ; BSY?
    jnz .wait_ready
    test al, 0x40       ; DRDY?
    jz .wait_ready

    ; Send IDENTIFY DEVICE command
    mov dx, 0x1F7
    mov al, 0xEC
    out dx, al

    ; Check for ATAPI signature in cylinder regs
    mov dx, 0x1F4       ; LBA mid (cylinder low)
    in al, dx
    cmp al, 0x14
    jne .wait_drq

    mov dx, 0x1F5       ; LBA high (cylinder high)
    in al, dx
    cmp al, 0xEB
    jne .wait_drq

    ; ATAPI device detected
    mov si, msg_atapi
    call print_str
    jmp .done

.wait_drq:
    mov dx, 0x1F7
    in al, dx
    test al, 0x80       ; BSY?
    jnz .wait_drq
    test al, 0x08       ; DRQ?
    jz .error

    ; Read 256 words = 512 bytes; model string is at words 27-46 (offset 54-93)
    mov cx, 256
    mov di, ata_buf
.read_loop:
    mov dx, 0x1F0
    in ax, dx
    mov [di], ax
    add di, 2
    loop .read_loop

    ; Print model string (words 27-46 = bytes 54-93, 40 bytes, byte-swapped pairs)
    mov si, msg_model
    call print_str

    mov si, ata_buf + 54    ; word 27 starts at byte 54
    mov cx, 40
.print_model:
    ; ATA strings are byte-swapped per word: [odd_char, even_char]
    lodsb                   ; Load odd-indexed byte (actually second char)
    mov bl, al
    lodsb                   ; Load even-indexed byte (actually first char)
    ; Print first char (al), then second char (bl)
    mov ah, 0x0E
    push bx
    push cx
    push si
    int 0x10
    mov al, bl
    mov ah, 0x0E
    int 0x10
    pop si
    pop cx
    pop bx
    sub cx, 2
    jnz .print_model

    mov si, msg_ok
    call print_str
    jmp .done

.error:
    mov si, msg_error
    call print_str

.done:
    ; Wait for keypress then exit
    mov ah, 0x00
    int 0x16
    int 0x20

print_str:
    lodsb
    or al, al
    jz .done
    mov ah, 0x0E
    int 0x10
    jmp print_str
.done:
    ret

msg_atapi  db 'ATAPI device detected on primary master', 13, 10, 0
msg_model  db 'ATA Model: ', 0
msg_ok     db 13, 10, 'IDENTIFY OK', 13, 10, 0
msg_error  db 'No DRQ after IDENTIFY', 13, 10, 0

ata_buf times 512 db 0
