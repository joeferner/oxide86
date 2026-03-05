; =============================================================================
; INT 13h Hard Disk Service Tests – Verify
; CPU: 286  Target: Drive C: (hard disk, DL=80h)
; Functions tested: 04h
;
; Exit codes (INT 21h / AH=4Ch):
;   AL = 0x00  all tests passed
;   AL = 0x01  Function 04h (Verify Sectors) failed
; =============================================================================

[CPU 286]
org 0x0100

start:
    ; =========================================================================
    ; TEST 1 – Function 04h: Verify Sectors
    ; Verify 1 sector at Cylinder 0, Head 0, Sector 1
    ; AH=04h, AL=01h (count), CH=00h (cylinder), CL=01h (sector),
    ; DH=00h (head), DL=80h (drive C:)
    ; On success: carry clear, AH=00h, AL=01h (sectors verified)
    ; =========================================================================
test_verify_sectors:
    mov  ah, 0x04
    mov  al, 0x01          ; verify 1 sector
    mov  ch, 0x00          ; cylinder 0
    mov  cl, 0x01          ; sector 1
    mov  dh, 0x00          ; head 0
    mov  dl, 0x80          ; drive C:
    int  0x13
    jc   fail_verify
    cmp  ah, 0x00
    jne  fail_verify
    cmp  al, 0x01          ; 1 sector must be confirmed
    jne  fail_verify

all_pass:
    mov  ah, 0x4C
    mov  al, 0x00
    int  0x21

fail_verify:
    mov  ah, 0x4C
    mov  al, 0x01
    int  0x21
