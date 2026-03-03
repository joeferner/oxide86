; INT 1Ah Function 04h - Get RTC Date
; Expected: 3/2/2026
; CH = BCD century (0x20), CL = BCD year (0x26)
; DH = BCD month (0x03), DL = BCD day (0x02)
; Carry flag set if RTC not operating

[CPU 8086]
org 0x0100

start:
    mov ah, 0x04        ; Function 04h: get real-time clock date
    int 0x1A

    ; Carry set means clock not running
    jc  fail

    ; Verify century = 0x20 (BCD for "20")
    cmp ch, 0x20
    jne fail

    ; Verify year = 0x26 (BCD for "26")
    cmp cl, 0x26
    jne fail

    ; Verify month = 0x03 (BCD for March)
    cmp dh, 0x03
    jne fail

    ; Verify day = 0x02 (BCD for 2nd)
    cmp dl, 0x02
    jne fail

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
