; =============================================================================
; INT 13h Hard Disk Service Tests
; CPU: 286  Target: Drive C: (hard disk, DL=80h)
; Functions tested: 00h 01h 02h 08h 15h
;
; Exit codes (INT 21h / AH=4Ch):
;   AL = 0x00  all tests passed
;   AL = 0x01  Function 00h (Reset) failed
;   AL = 0x02  Function 01h (Get Status) failed
;   AL = 0x03  Function 02h (Read Sectors) failed
;   AL = 0x04  Function 08h (Get Drive Parameters) failed
;   AL = 0x05  Function 15h (Get Drive Type) failed
; =============================================================================

[CPU 286]
org 0x0100

start:
    ; =========================================================================
    ; TEST 1 – Function 00h: Reset Disk System
    ; AH=00h, DL=80h (drive C:)
    ; On success: carry clear, AH=00h
    ; =========================================================================
test_reset:
    mov  ah, 0x00
    mov  dl, 0x80          ; drive C:
    int  0x13
    jc   fail_reset        ; carry set → error
    cmp  ah, 0x00          ; AH must be 0 on success
    jne  fail_reset

    ; =========================================================================
    ; TEST 2 – Function 01h: Get Last Drive Status
    ; AH=01h, DL=80h
    ; After a successful reset the status byte must be 0x00 (no error)
    ; =========================================================================
test_get_status:
    mov  ah, 0x01
    mov  dl, 0x80
    int  0x13
    cmp  ah, 0x00
    jne  fail_get_status

    ; =========================================================================
    ; TEST 3 – Function 02h: Read Sectors
    ; Read 1 sector from Cylinder 0, Head 0, Sector 1
    ; AH=02h, AL=01h (sector count), CH=00h (cylinder low), CL=01h (sector),
    ; DH=00h (head), DL=80h (drive C:), ES:BX → disk_buf
    ; On success: carry clear, AH=00h, AL=sectors actually read (must be 1)
    ; =========================================================================
test_read_sectors:
    mov  bx, disk_buf
    mov  ah, 0x02
    mov  al, 0x01          ; read 1 sector
    mov  ch, 0x00          ; cylinder 0 (low byte)
    mov  cl, 0x01          ; sector 1, cylinder high bits = 0
    mov  dh, 0x00          ; head 0
    mov  dl, 0x80          ; drive C:
    int  0x13
    jc   fail_read_sectors
    cmp  ah, 0x00
    jne  fail_read_sectors
    cmp  al, 0x01          ; verify 1 sector was transferred
    jne  fail_read_sectors

    ; =========================================================================
    ; TEST 4 – Function 08h: Get Drive Parameters
    ; AH=08h, DL=80h
    ; On success: carry clear, AH=00h
    ;   CH = max cylinder (low 8 bits), CL[7:6] = max cylinder high 2 bits,
    ;        CL[5:0] = sectors per track
    ;   DH = max head index (0-based), DL = number of hard drives
    ; Sanity checks: sectors-per-track (CL & 3Fh) must be non-zero,
    ; and head count (DH) must be at least 1 (i.e. 2+ heads).
    ; =========================================================================
test_get_params:
    mov  ah, 0x08
    mov  dl, 0x80
    int  0x13
    jc   fail_get_params
    cmp  ah, 0x00
    jne  fail_get_params

    ; sectors per track in CL bits [5:0]
    mov  al, cl
    and  al, 0x3F          ; mask out cylinder high bits
    cmp  al, 0x00
    je   fail_get_params   ; 0 sectors/track is nonsensical

    ; hard drive must have at least 2 heads (DH is 0-based max head, so >= 1)
    cmp  dh, 0x01
    jb   fail_get_params   ; fewer than 2 heads → wrong drive type

    ; =========================================================================
    ; TEST 5 – Function 15h: Get Drive Type
    ; AH=15h, DL=80h
    ; On success: carry clear
    ;   AH = 03h  fixed disk (hard drive)
    ;   AH = 00h  drive not present (fail)
    ;   AH = 01h  floppy, no change-line (unexpected for 80h)
    ;   AH = 02h  floppy, change-line (unexpected for 80h)
    ; =========================================================================
test_get_drive_type:
    mov  ah, 0x15
    mov  dl, 0x80
    int  0x13
    jc   fail_get_drive_type
    cmp  ah, 0x03          ; must be fixed disk
    jne  fail_get_drive_type

    ; =========================================================================
    ; All tests passed
    ; =========================================================================
all_pass:
    mov  ah, 0x4C
    mov  al, 0x00
    int  0x21

    ; =========================================================================
    ; Individual failure exits – unique AL code for each failing test
    ; =========================================================================
fail_reset:
    mov  ah, 0x4C
    mov  al, 0x01
    int  0x21

fail_get_status:
    mov  ah, 0x4C
    mov  al, 0x02
    int  0x21

fail_read_sectors:
    mov  ah, 0x4C
    mov  al, 0x03
    int  0x21

fail_get_params:
    mov  ah, 0x4C
    mov  al, 0x04
    int  0x21

fail_get_drive_type:
    mov  ah, 0x4C
    mov  al, 0x05
    int  0x21

; =============================================================================
; 512-byte scratch buffer for sector reads (one full sector)
; =============================================================================
disk_buf:
    times 512 db 0x00
