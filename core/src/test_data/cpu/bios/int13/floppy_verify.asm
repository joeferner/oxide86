; =============================================================================
; INT 13h Floppy Disk Service Tests – Verify & DASD
; CPU: 286  Target: Drive A: (floppy inserted, 1.44MB)
; Functions tested: 04h 18h
;
; Exit codes (INT 21h / AH=4Ch):
;   AL = 0x00  all tests passed
;   AL = 0x01  Function 04h (Verify Sectors) failed
;   AL = 0x02  Function 18h (Set DASD Type) failed
; =============================================================================

[CPU 286]
org 0x0100

start:
    ; =========================================================================
    ; TEST 1 – Function 04h: Verify Sectors
    ; Verify 1 sector at Cylinder 0, Head 0, Sector 1
    ; AH=04h, AL=01h (count), CH=00h (cylinder), CL=01h (sector),
    ; DH=00h (head), DL=00h (drive A:)
    ; On success: carry clear, AH=00h, AL=01h (sectors verified)
    ; =========================================================================
test_verify_sectors:
    mov  ah, 0x04
    mov  al, 0x01          ; verify 1 sector
    mov  ch, 0x00          ; cylinder 0
    mov  cl, 0x01          ; sector 1
    mov  dh, 0x00          ; head 0
    mov  dl, 0x00          ; drive A:
    int  0x13
    jc   fail_verify
    cmp  ah, 0x00
    jne  fail_verify
    cmp  al, 0x01          ; 1 sector must be confirmed
    jne  fail_verify

    ; =========================================================================
    ; TEST 2 – Function 18h: Set DASD Type for Format
    ; Configure for 1.44MB floppy: 80 tracks, 18 sectors/track
    ; AH=18h, CH=0x4F (79 = max track index), CL=0x12 (18 spt), DL=00h
    ; On success: carry clear, AH=00h, ES:DI = Disk Base Table pointer
    ; =========================================================================
test_set_dasd_type:
    mov  ah, 0x18
    mov  ch, 0x4F          ; 79 = 0-based max track for an 80-track floppy
    mov  cl, 0x12          ; 18 sectors per track
    mov  dl, 0x00          ; drive A:
    int  0x13
    jc   fail_dasd
    cmp  ah, 0x00
    jne  fail_dasd

    ; Verify ES:DI = F000:E000 (our BIOS ROM DBT location)
    mov  ax, es
    cmp  ax, 0xF000
    jne  fail_dasd
    cmp  di, 0xE000
    jne  fail_dasd

all_pass:
    mov  ah, 0x4C
    mov  al, 0x00
    int  0x21

fail_verify:
    mov  ah, 0x4C
    mov  al, 0x01
    int  0x21

fail_dasd:
    mov  ah, 0x4C
    mov  al, 0x02
    int  0x21
