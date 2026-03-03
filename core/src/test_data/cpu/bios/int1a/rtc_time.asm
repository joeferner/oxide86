; INT 1Ah Function 02h - Get RTC Time
; Expected: 11:05 AM → BCD hour = 0x11, BCD minute = 0x05
; CH = BCD hours, CL = BCD minutes, DH = BCD seconds, DL = daylight flag
; Carry flag set if RTC not operating

[CPU 8086]
org 0x0100

start:
    mov ah, 0x02        ; Function 02h: get real-time clock time
    int 0x1A

    ; Carry set means clock not running / not set
    jc  fail

    ; Verify hour = 0x11 (BCD for 11)
    cmp ch, 0x11
    jne fail

    ; Verify minute = 0x05 (BCD for 05)
    cmp cl, 0x05
    jne fail

    ; Seconds (DH) not checked - too fast-moving to pin down
    ; Daylight flag (DL): 0x01 = DST in effect, 0x00 = standard - accept either
    ; (no check on DL)

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
