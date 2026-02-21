; ATAPI READ TOC test
; Issues PACKET command + READ TOC CDB to the primary slave (CD-ROM)
; Run: cargo run -p oxide86-native-cli -- --cdrom game.iso test-programs/cdrom/atapi_read_toc.com
;
; Expected: prints first track start LBA (should be 0) and lead-out LBA

org 0x100

start:
    ; Select slave device (DEV=1)
    mov dx, 0x1F6
    mov al, 0xB0
    out dx, al

    ; 400ns delay via alt-status reads
    mov dx, 0x3F6
    in al, dx
    in al, dx
    in al, dx
    in al, dx

.wait_ready:
    mov dx, 0x1F7
    in al, dx
    test al, 0x80       ; BSY?
    jnz .wait_ready

    ; Check device exists (DRDY should be set for ATAPI too)
    test al, 0x40
    jz .no_device

    ; Set byte count limit for ATAPI transfer (0x0014 = 20 bytes for TOC)
    mov dx, 0x1F4
    mov al, 0x14        ; Byte count low
    out dx, al
    mov dx, 0x1F5
    mov al, 0x00        ; Byte count high
    out dx, al

    ; Issue PACKET command (0xA0)
    mov dx, 0x1F7
    mov al, 0xA0
    out dx, al

    ; Wait for DRQ (device wants the 12-byte CDB)
.wait_cdb_drq:
    mov dx, 0x1F7
    in al, dx
    test al, 0x80
    jnz .wait_cdb_drq
    test al, 0x08       ; DRQ?
    jz .error

    ; Send READ TOC CDB (12 bytes via word writes to 0x1F0)
    ; CDB: 43 02 00 00 00 00 00 00 14 00 00 00
    ;      cmd  msf                 alloc=20
    mov dx, 0x1F0
    mov ax, 0x0243      ; CDB[0]=0x43 (READ TOC), CDB[1]=0x02 (MSF bit set)
    out dx, ax
    mov ax, 0x0000      ; CDB[2]=0, CDB[3]=0
    out dx, ax
    mov ax, 0x0000      ; CDB[4]=0, CDB[5]=0
    out dx, ax
    mov ax, 0x0000      ; CDB[6]=0, CDB[7]=0
    out dx, ax
    mov ax, 0x0014      ; CDB[8]=0x14(20), CDB[9]=0
    out dx, ax
    mov ax, 0x0000      ; CDB[10]=0, CDB[11]=0
    out dx, ax

    ; Wait for data DRQ
.wait_data_drq:
    mov dx, 0x1F7
    in al, dx
    test al, 0x80
    jnz .wait_data_drq
    test al, 0x08
    jz .error

    ; Read 10 words (20 bytes) from data port
    mov cx, 10
    mov di, toc_buf
.read_loop:
    mov dx, 0x1F0
    in ax, dx
    mov [di], ax
    add di, 2
    loop .read_loop

    ; Parse TOC buffer:
    ; Byte 0-1: TOC data length
    ; Byte 2: First track, Byte 3: Last track
    ; Track 1 descriptor at offset 4: reserved, ADR/CTL, track#, reserved, MSF[0..3]
    ; Lead-out descriptor at offset 12: same, track=0xAA
    mov si, msg_toc
    call print_str

    ; Print first track start MSF (bytes 8-10 of toc_buf)
    mov al, [toc_buf + 9]   ; M
    call print_hex
    mov al, ':'
    mov ah, 0x0E
    int 0x10
    mov al, [toc_buf + 10]  ; S
    call print_hex
    mov al, ':'
    mov ah, 0x0E
    int 0x10
    mov al, [toc_buf + 11]  ; F
    call print_hex

    mov si, msg_crlf
    call print_str

    ; Print lead-out MSF
    mov si, msg_leadout
    call print_str
    mov al, [toc_buf + 17]  ; M
    call print_hex
    mov al, ':'
    mov ah, 0x0E
    int 0x10
    mov al, [toc_buf + 18]  ; S
    call print_hex
    mov al, ':'
    mov ah, 0x0E
    int 0x10
    mov al, [toc_buf + 19]  ; F
    call print_hex
    mov si, msg_crlf
    call print_str
    jmp .done

.no_device:
    mov si, msg_nodev
    call print_str
    jmp .done

.error:
    mov si, msg_error
    call print_str

.done:
    mov ah, 0x00
    int 0x16
    int 0x20

; Print AL as 2-digit hex
print_hex:
    push ax
    push bx
    push cx
    mov bl, al
    shr al, 4
    call .nibble
    mov al, bl
    and al, 0x0F
    call .nibble
    pop cx
    pop bx
    pop ax
    ret
.nibble:
    add al, '0'
    cmp al, '9'
    jle .ok
    add al, 7
.ok:
    mov ah, 0x0E
    int 0x10
    ret

print_str:
    lodsb
    or al, al
    jz .done
    mov ah, 0x0E
    int 0x10
    jmp print_str
.done:
    ret

msg_toc     db 'TOC Track 1 MSF: ', 0
msg_leadout db 'Lead-out MSF:    ', 0
msg_crlf    db 13, 10, 0
msg_nodev   db 'No device on primary slave', 13, 10, 0
msg_error   db 'ATAPI command error', 13, 10, 0

toc_buf times 20 db 0
