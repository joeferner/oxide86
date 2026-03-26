; RTC detection test: verify that CMOS is present and functional
;
; Checks:
;   1. Status Register D (0x0D) returns 0x80 (VRT bit set, reserved bits clear)
;   2. Alarm seconds register (0x01) can be written and read back correctly
;
; Exit 0 on success, exit 1 on the first failure.

[CPU 8086]
org 0x0100

start:
    ; Test 1: Status Register D should return 0x80 (battery valid, bits 6-0 = 0)
    mov al, 0x8D        ; 0x80 = NMI disable, 0x0D = Status D
    out 0x70, al
    in  al, 0x71
    cmp al, 0x80
    jne fail

    ; Test 2: Write 0x55 to seconds alarm register (0x01), read back
    mov al, 0x81        ; NMI disable | 0x01 seconds alarm
    out 0x70, al
    mov al, 0x55
    out 0x71, al

    mov al, 0x81
    out 0x70, al
    in  al, 0x71
    cmp al, 0x55
    jne fail

    ; Test 3: Write 0xAA, verify different value reads back correctly
    mov al, 0x81
    out 0x70, al
    mov al, 0xAA
    out 0x71, al

    mov al, 0x81
    out 0x70, al
    in  al, 0x71
    cmp al, 0xAA
    jne fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
