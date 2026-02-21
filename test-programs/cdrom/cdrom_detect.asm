; cdrom_detect.asm - MSCDEX CD-ROM detection and sector read test
; Run with: cargo run -p emu86-native-cli -- --cdrom my_disk.iso test-programs/cdrom/cdrom_detect.com
; Build:    nasm -f bin cdrom_detect.asm -o cdrom_detect.com

org 100h

section .text

start:
    ; ----------------------------------------------------------------
    ; Step 1: MSCDEX install check (INT 2Fh AX=1500h)
    ;   BX = number of CD-ROM drives on return
    ;   AX = ADAD if installed
    ; ----------------------------------------------------------------
    mov ax, 1500h
    xor bx, bx
    int 2Fh

    cmp ax, 0ADADh
    jne .not_found

    cmp bx, 0
    je  .not_found

    ; ----------------------------------------------------------------
    ; Step 2: Print detection message
    ; ----------------------------------------------------------------
    mov dx, msg_found
    mov ah, 09h
    int 21h

    ; Print drive count as single digit (BX holds count, 1-9 expected)
    mov ax, bx
    add al, '0'
    mov dl, al
    mov ah, 02h
    int 21h

    mov dx, msg_drives
    mov ah, 09h
    int 21h

    ; ----------------------------------------------------------------
    ; Step 3: Query CD-ROM version (INT 2Fh AX=150Ch)
    ;   BX = version (major in BH, minor in BL)
    ; ----------------------------------------------------------------
    mov ax, 150Ch
    int 2Fh

    mov dx, msg_version
    mov ah, 09h
    int 21h

    ; Print major version
    mov al, bh
    add al, '0'
    mov dl, al
    mov ah, 02h
    int 21h

    mov dl, '.'
    mov ah, 02h
    int 21h

    ; Print minor version as two digits
    mov al, bl
    mov ah, 0
    mov bl, 10
    div bl          ; AL = tens, AH = ones
    add al, '0'
    mov dl, al
    mov ah, 02h
    int 21h
    add ah, '0'
    mov dl, ah
    mov ah, 02h
    int 21h

    mov dx, msg_crlf
    mov ah, 09h
    int 21h

    ; ----------------------------------------------------------------
    ; Step 4: Read sector 16 (Primary Volume Descriptor) via INT 13h
    ;   Drive 0xE0, AH=02h, AL=1 sector, CH=0,CL=sector, DH=head
    ;   LBA 16 in CHS terms: cylinder=0, head=0, sector=17 (1-based)
    ;   Note: CD-ROM uses 2048-byte sectors; INT 13h sectors are 512 bytes
    ;         LBA 16 (2048-byte) = LBA 64 (512-byte); CHS sector = 65 (1-based)
    ; ----------------------------------------------------------------
    mov dx, msg_read
    mov ah, 09h
    int 21h

    mov ax, 0201h       ; AH=02 (read), AL=1 sector
    mov bx, sector_buf  ; ES:BX = buffer
    mov cx, 0041h       ; CH=0 (cylinder), CL=0x41=65 (sector, 1-based)
    mov dx, 00E0h       ; DH=0 (head), DL=0xE0 (CD-ROM drive 0)
    int 13h

    jc  .read_error

    ; Check for ISO 9660 signature at bytes 1-5 of sector 16
    ; PVD identifier: "CD001" at offset 1
    mov si, sector_buf + 1
    mov di, iso_sig
    mov cx, 5
    repe cmpsb
    jne .bad_sig

    mov dx, msg_iso_ok
    mov ah, 09h
    int 21h
    jmp .done

.bad_sig:
    mov dx, msg_bad_sig
    mov ah, 09h
    int 21h
    jmp .done

.read_error:
    mov dx, msg_read_err
    mov ah, 09h
    int 21h
    jmp .done

.not_found:
    mov dx, msg_not_found
    mov ah, 09h
    int 21h

.done:
    mov ax, 4C00h
    int 21h

; ----------------------------------------------------------------
section .data

msg_found:    db 'MSCDEX detected: $'
msg_drives:   db ' CD-ROM drive(s)', 0Dh, 0Ah, '$'
msg_version:  db 'MSCDEX version: $'
msg_crlf:     db 0Dh, 0Ah, '$'
msg_read:     db 'Reading ISO 9660 PVD (sector 16)... $'
msg_iso_ok:   db 'OK - CD001 signature found', 0Dh, 0Ah, '$'
msg_bad_sig:  db 'FAIL - wrong signature', 0Dh, 0Ah, '$'
msg_read_err: db 'FAIL - INT 13h read error', 0Dh, 0Ah, '$'
msg_not_found:db 'No CD-ROM / MSCDEX not found', 0Dh, 0Ah, '$'
iso_sig:      db 'CD001'

section .bss

sector_buf:   resb 512
