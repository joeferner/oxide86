; INT 1Ah Function 03h / 05h - Set RTC Time and Date, then verify via 02h / 04h
; Sets RTC to 3/2/2026 11:05:30 AM and reads back to confirm
; All values are BCD encoded throughout

[CPU 8086]
org 0x0100

start:
    ; --- Part 1: Set the RTC date (Function 05h) ---
    mov ah, 0x05
    mov ch, 0x20        ; BCD century = 20
    mov cl, 0x26        ; BCD year    = 26
    mov dh, 0x03        ; BCD month   = 03 (March)
    mov dl, 0x02        ; BCD day     = 02
    int 0x1A

    ; --- Part 2: Set the RTC time (Function 03h) ---
    mov ah, 0x03
    mov ch, 0x11        ; BCD hours   = 11
    mov cl, 0x05        ; BCD minutes = 05
    mov dh, 0x30        ; BCD seconds = 30
    mov dl, 0x00        ; standard time (not DST)
    int 0x1A

    ; --- Part 3: Read back and verify the date (Function 04h) ---
    mov ah, 0x04
    int 0x1A

    ; Carry set means RTC not operating
    jc  fail

    cmp ch, 0x20        ; century = 20?
    jne fail
    cmp cl, 0x26        ; year    = 26?
    jne fail
    cmp dh, 0x03        ; month   = March?
    jne fail
    cmp dl, 0x02        ; day     = 2nd?
    jne fail

    ; --- Part 4: Read back and verify the time (Function 02h) ---
    mov ah, 0x02
    int 0x1A

    ; Carry set means RTC not operating
    jc  fail

    cmp ch, 0x11        ; hour   = 11?
    jne fail
    cmp cl, 0x05        ; minute = 05?
    jne fail

    ; Verify seconds are >= 0x30 (time is always advancing, never goes backward)
    ; and have not advanced more than 0x08 seconds since we set them
    ; Using BCD-safe comparison: 0x30 to 0x38
    cmp dh, 0x30
    jb  fail
    cmp dh, 0x38
    ja  fail

    ; DST flag (DL): accept either 0x00 or 0x01 - no check needed

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
