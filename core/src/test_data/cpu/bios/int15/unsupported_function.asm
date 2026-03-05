; INT 15h - Unsupported function
; AH=41h (Wait for External Event) is PS/2-only and not available on 8086.
; The BIOS must set CF=1 to signal "function not supported".

[CPU 8086]
org 0x0100

start:
    ; Clear carry before the call to confirm the BIOS sets it
    clc

    mov ah, 0x41        ; Function 41h: Wait for External Event (PS/2 only)
    int 0x15

    ; CF must be set (function not supported)
    jnc fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
