; =============================================================================
; INT 13h Read-Only Floppy Write-Protect Test
; CPU: 286  Target: Drive A: (read-only floppy inserted)
; Functions tested: 00h 03h
;
; Test sequence:
;   1. Reset disk (00h)
;   2. Attempt to write 1 sector to C=0, H=0, S=1 (03h)
;      This must FAIL with carry set and AH=03h (Write Protected)
;
; Exit codes (INT 21h / AH=4Ch):
;   AL = 0x00  write correctly rejected with write-protect error
;   AL = 0x01  Function 00h (Reset) failed
;   AL = 0x02  Write succeeded unexpectedly (disk should be read-only)
;   AL = 0x03  Write failed but with wrong error code (expected AH=03h)
; =============================================================================

[CPU 286]
org 0x0100

start:
    ; =========================================================================
    ; TEST 1 – Function 00h: Reset Disk System
    ; =========================================================================
test_reset:
    mov  ah, 0x00
    mov  dl, 0x00          ; drive A:
    int  0x13
    jc   fail_reset
    cmp  ah, 0x00
    jne  fail_reset

    ; =========================================================================
    ; TEST 2 – Function 03h: Write Sectors (must be rejected)
    ; Write 1 sector from write_buf to C=0, H=0, S=1
    ; Expected: CF=1, AH=03h (Write Protected)
    ; =========================================================================
test_write:
    mov  bx, write_buf
    mov  ah, 0x03
    mov  al, 0x01          ; write 1 sector
    mov  ch, 0x00          ; cylinder 0
    mov  cl, 0x01          ; sector 1
    mov  dh, 0x00          ; head 0
    mov  dl, 0x00          ; drive A:
    int  0x13
    jnc  fail_write_succeeded  ; carry must be set (error)
    cmp  ah, 0x03              ; AH must be 03h (Write Protected)
    jne  fail_wrong_error

    ; =========================================================================
    ; All tests passed
    ; =========================================================================
all_pass:
    mov  ah, 0x4C
    mov  al, 0x00
    int  0x21

    ; =========================================================================
    ; Individual failure exits
    ; =========================================================================
fail_reset:
    mov  ah, 0x4C
    mov  al, 0x01
    int  0x21

fail_write_succeeded:
    mov  ah, 0x4C
    mov  al, 0x02
    int  0x21

fail_wrong_error:
    mov  ah, 0x4C
    mov  al, 0x03
    int  0x21

; =============================================================================
; Buffer
; write_buf: 512 bytes filled with 0xA5 (the pattern that must not be written)
; =============================================================================
write_buf:
    times 512 db 0xA5
