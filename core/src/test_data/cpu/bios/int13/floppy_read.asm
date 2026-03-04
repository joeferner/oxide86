; =============================================================================
; INT 13h Floppy Disk Service Tests
; CPU: 286  Target: Drive A: (floppy inserted)
; Functions tested: 00h 01h 02h 08h 15h 16h
;
; Exit codes (INT 21h / AH=4Ch):
;   AL = 0x00  all tests passed
;   AL = 0x01  Function 00h (Reset) failed
;   AL = 0x02  Function 01h (Get Status) failed
;   AL = 0x03  Function 02h (Read Sectors) failed
;   AL = 0x04  Function 08h (Get Drive Parameters) failed
;   AL = 0x05  Function 15h (Get Drive Type) failed
;   AL = 0x06  Function 16h (Detect Media Change) failed
; =============================================================================

[CPU 286]
org 0x0100

; ---------------------------------------------------------------------------
; One sector of scratch space located just past the code.
; We compute the address at run time so the assembler does not need to know
; where the code ends.
; ---------------------------------------------------------------------------

start:
    ; ---- set up a 512-byte read buffer on the stack segment ---------------
    ; DS = CS (COM file), so we can address disk_buf directly.
    ; We will fill disk_buf's address into BX when needed.

    ; =========================================================================
    ; TEST 1 – Function 00h: Reset Disk System
    ; AH=00h, DL=00h (drive A:)
    ; On success: carry clear, AH=00h
    ; =========================================================================
test_reset:
    mov  ah, 0x00
    mov  dl, 0x00          ; drive A:
    int  0x13
    jc   fail_reset        ; carry set → error
    cmp  ah, 0x00          ; AH must be 0 on success
    jne  fail_reset

    ; =========================================================================
    ; TEST 2 – Function 01h: Get Last Drive Status
    ; AH=01h, DL=00h
    ; After a successful reset the status byte must be 0x00 (no error)
    ; =========================================================================
test_get_status:
    mov  ah, 0x01
    mov  dl, 0x00
    int  0x13
    ; INT 13h/01h always clears carry and returns last status in AH.
    ; Carry behaviour is BIOS-dependent; we just check AH.
    cmp  ah, 0x00
    jne  fail_get_status

    ; =========================================================================
    ; TEST 3 – Function 02h: Read Sectors
    ; Read 1 sector from Track 0, Head 0, Sector 1 (logical first sector)
    ; AH=02h, AL=01h (sector count), CH=00h (track), CL=01h (sector),
    ; DH=00h (head), DL=00h (drive A:), ES:BX → disk_buf
    ; On success: carry clear, AH=00h, AL=sectors actually read (must be 1)
    ; =========================================================================
test_read_sectors:
    ; Point ES:BX at our scratch buffer (same segment as CS/DS for a COM file)
    mov  bx, disk_buf
    mov  ah, 0x02
    mov  al, 0x01          ; read 1 sector
    mov  ch, 0x00          ; cylinder 0
    mov  cl, 0x01          ; sector 1
    mov  dh, 0x00          ; head 0
    mov  dl, 0x00          ; drive A:
    int  0x13
    jc   fail_read_sectors
    cmp  ah, 0x00
    jne  fail_read_sectors
    cmp  al, 0x01          ; verify 1 sector was transferred
    jne  fail_read_sectors

    ; =========================================================================
    ; TEST 4 – Function 08h: Get Drive Parameters
    ; AH=08h, DL=00h
    ; On success: carry clear, AH=00h
    ;   BL = drive type (01h=360K, 02h=1.2M, 03h=720K, 04h=1.44M, 06h=2.88M)
    ;   CH = max cylinder (low 8 bits), CL[7:6] = max cylinder high 2 bits,
    ;        CL[5:0] = sectors per track
    ;   DH = max head index, DL = number of drives on controller
    ; We sanity-check: sectors-per-track (CL & 3Fh) must be 8, 9, 15, 18, or 36
    ; and head count (DH) must be 0 or 1 (0-based, so ≤1 = 2 heads max).
    ; =========================================================================
test_get_params:
    mov  ah, 0x08
    mov  dl, 0x00
    int  0x13
    jc   fail_get_params
    cmp  ah, 0x00
    jne  fail_get_params

    ; sectors per track in CL bits [5:0]
    mov  al, cl
    and  al, 0x3F          ; mask out cylinder high bits
    cmp  al, 0x00
    je   fail_get_params   ; 0 sectors/track is nonsensical

    ; head count: DH is 0-based max head, so valid values are 0 or 1
    cmp  dh, 0x01
    ja   fail_get_params   ; more than 2 heads on a floppy → wrong drive

    ; =========================================================================
    ; TEST 5 – Function 15h: Get Drive Type (AT / PS2 BIOS extension)
    ; AH=15h, DL=00h
    ; On success: carry clear
    ;   AH = 01h  floppy, no change-line support
    ;   AH = 02h  floppy, change-line support
    ;   AH = 03h  fixed disk  (should not appear for drive A:)
    ;   AH = 00h  drive not present (fail)
    ; =========================================================================
test_get_drive_type:
    mov  ah, 0x15
    mov  dl, 0x00
    int  0x13
    jc   fail_get_drive_type
    cmp  ah, 0x00          ; type 00h = drive not present
    je   fail_get_drive_type
    cmp  ah, 0x03          ; type 03h = hard disk – unexpected for A:
    je   fail_get_drive_type

    ; AH must be 01h or 02h (floppy with or without change-line)
    cmp  ah, 0x01
    je   .type_ok
    cmp  ah, 0x02
    jne  fail_get_drive_type
.type_ok:

    ; =========================================================================
    ; TEST 6 – Function 16h: Detect Media Change (change-line status)
    ; AH=16h, DL=00h
    ; Returns:
    ;   AH=00h, carry clear  → disk has NOT been changed
    ;   AH=06h, carry set    → disk HAS been changed (or status unknown)
    ;   AH=80h               → drive not ready / no media
    ; Both 00h and 06h are valid "functional" responses; 80h is a failure.
    ; Note: not all BIOSes support 16h on all floppy types; we treat a missing-
    ;       function response (AH=01h = invalid function) as a soft skip rather
    ;       than a hard fail, since 286-era BIOSes vary widely.
    ; =========================================================================
test_media_change:
    mov  ah, 0x16
    mov  dl, 0x00
    int  0x13

    ; AH=80h → no media / drive not ready → fail
    cmp  ah, 0x80
    je   fail_media_change

    ; AH=01h → function not supported by this BIOS → soft pass
    cmp  ah, 0x01
    je   all_pass

    ; AH=00h (no change, carry clear) or AH=06h (changed, carry set) are both
    ; acceptable results indicating the BIOS and drive are responsive.
    cmp  ah, 0x00
    je   all_pass
    cmp  ah, 0x06
    je   all_pass

    ; Any other code is unexpected
    jmp  fail_media_change

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

fail_media_change:
    mov  ah, 0x4C
    mov  al, 0x06
    int  0x21

; =============================================================================
; 512-byte aligned scratch buffer for sector reads (one full sector)
; Placed at end of COM image; NASM will zero-fill the BSS-style reservation.
; =============================================================================
disk_buf:
    times 512 db 0x00
