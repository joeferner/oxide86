; =============================================================================
; INT 13h Floppy Disk Write Service Test
; CPU: 286  Target: Drive A: (floppy inserted)
; Functions tested: 00h 03h 02h
;
; Test sequence:
;   1. Reset disk (00h)
;   2. Write 1 sector with 0xA5 pattern to C=0, H=0, S=1 (03h)
;   3. Read the sector back into a separate buffer (02h)
;   4. Verify the first and last bytes of the read buffer match 0xA5
;
; Exit codes (INT 21h / AH=4Ch):
;   AL = 0x00  all tests passed
;   AL = 0x01  Function 00h (Reset) failed
;   AL = 0x02  Function 03h (Write Sectors) failed
;   AL = 0x03  Function 02h (Read Back) failed
;   AL = 0x04  Data verify failed (read-back data does not match written data)
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
    ; TEST 2 – Function 03h: Write Sectors
    ; Write 1 sector from write_buf (filled with 0xA5) to C=0, H=0, S=1
    ; AH=03h, AL=01h, CH=00h, CL=01h, DH=00h, DL=00h, ES:BX → write_buf
    ; On success: carry clear, AH=00h, AL=01h
    ; =========================================================================
test_write_sectors:
    mov  bx, write_buf
    mov  ah, 0x03
    mov  al, 0x01          ; write 1 sector
    mov  ch, 0x00          ; cylinder 0
    mov  cl, 0x01          ; sector 1
    mov  dh, 0x00          ; head 0
    mov  dl, 0x00          ; drive A:
    int  0x13
    jc   fail_write_sectors
    cmp  ah, 0x00
    jne  fail_write_sectors
    cmp  al, 0x01          ; verify 1 sector was transferred
    jne  fail_write_sectors

    ; =========================================================================
    ; TEST 3 – Function 02h: Read Back the Written Sector
    ; Read into read_buf (initially all zeros) and verify contents
    ; =========================================================================
test_read_back:
    mov  bx, read_buf
    mov  ah, 0x02
    mov  al, 0x01          ; read 1 sector
    mov  ch, 0x00          ; cylinder 0
    mov  cl, 0x01          ; sector 1
    mov  dh, 0x00          ; head 0
    mov  dl, 0x00          ; drive A:
    int  0x13
    jc   fail_read_back
    cmp  ah, 0x00
    jne  fail_read_back
    cmp  al, 0x01
    jne  fail_read_back

    ; =========================================================================
    ; TEST 4 – Data Verify: check first and last bytes of read_buf
    ; Both must be 0xA5 (the pattern written in write_buf)
    ; =========================================================================
test_verify:
    mov  bx, read_buf
    cmp  byte [bx], 0xA5   ; first byte
    jne  fail_verify
    mov  bx, read_buf + 511
    cmp  byte [bx], 0xA5   ; last byte
    jne  fail_verify

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

fail_write_sectors:
    mov  ah, 0x4C
    mov  al, 0x02
    int  0x21

fail_read_back:
    mov  ah, 0x4C
    mov  al, 0x03
    int  0x21

fail_verify:
    mov  ah, 0x4C
    mov  al, 0x04
    int  0x21

; =============================================================================
; Buffers
; write_buf: 512 bytes filled with 0xA5 (the pattern to write to disk)
; read_buf:  512 bytes zeroed (destination for read-back verification)
; =============================================================================
write_buf:
    times 512 db 0xA5

read_buf:
    times 512 db 0x00
