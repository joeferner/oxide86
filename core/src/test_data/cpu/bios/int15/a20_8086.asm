; INT 15h AH=24h - A20 Gate on 8086
; The 8086 has only 20 address lines and no A20 gate hardware.
; All AH=24h subfunctions must return CF=1, AH=86h (unsupported).

[CPU 8086]
org 0x0100

start:
    ; ── Subfunction 01h: Enable A20 ─────────────────────────────────────────
    clc
    mov ax, 0x2401      ; AH=24h, AL=01h
    int 0x15
    jnc fail            ; CF must be set (unsupported)
    cmp ah, 0x86        ; AH must be 0x86 (function not supported)
    jne fail

    ; ── Subfunction 00h: Disable A20 ────────────────────────────────────────
    clc
    mov ax, 0x2400      ; AH=24h, AL=00h
    int 0x15
    jnc fail
    cmp ah, 0x86
    jne fail

    ; ── Subfunction 02h: Query A20 state ────────────────────────────────────
    clc
    mov ax, 0x2402      ; AH=24h, AL=02h
    int 0x15
    jnc fail
    cmp ah, 0x86
    jne fail

    ; ── Subfunction 03h: Query A20 support ──────────────────────────────────
    clc
    mov ax, 0x2403      ; AH=24h, AL=03h
    int 0x15
    jnc fail
    cmp ah, 0x86
    jne fail

    ; All tests passed
    mov ax, 0x4C00
    int 0x21

fail:
    mov ax, 0x4C01
    int 0x21
