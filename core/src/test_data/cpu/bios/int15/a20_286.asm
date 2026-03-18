; INT 15h AH=24h - A20 Gate on 286  (diagnostic version)
; Each fail_N exit encodes which check failed (exit code = step number).

[CPU 286]
org 0x0100

start:
    ; ── Step 1: Subfunction 03h — Query A20 support ─────────────────────────
    mov ax, 0x2403
    int 0x15
    jc  fail_1
    cmp ah, 0x00
    jne fail_1
    test bx, 0x0003
    jz  fail_1

    ; ── Step 2: Subfunction 01h — Enable A20 ────────────────────────────────
    mov ax, 0x2401
    int 0x15
    jc  fail_2
    cmp ah, 0x00
    jne fail_2

    ; ── Step 3: Query state — must report enabled (AL=1) ────────────────────
    mov ax, 0x2402
    int 0x15
    jc  fail_3
    cmp al, 0x01
    jne fail_3

    ; ── Step 4: A20 on — 0x000500 and 0x100500 must be distinct ─────────────
    ; Set up DS=0, ES=0xFFFF
    xor ax, ax
    mov ds, ax
    mov ax, 0xFFFF
    mov es, ax

    mov byte [ds:0x500], 0xAA   ; write 0xAA to linear 0x000500
    mov byte [es:0x510], 0xBB   ; write 0xBB to linear 0x100500
    cmp byte [ds:0x500], 0xAA   ; should still be 0xAA (different address)
    jne fail_4

    ; ── Step 5: Subfunction 00h — Disable A20 ───────────────────────────────
    mov ax, 0x2400
    int 0x15
    jc  fail_5
    cmp ah, 0x00
    jne fail_5

    ; ── Step 6: Query state — must report disabled (AL=0) ───────────────────
    mov ax, 0x2402
    int 0x15
    jc  fail_6
    cmp al, 0x00
    jne fail_6

    ; ── Step 7: A20 off — write through 0xFFFF:0x0510 must alias 0x000500 ───
    mov byte [ds:0x500], 0x11
    mov byte [es:0x510], 0x22   ; aliases linear 0x000500 with A20 off
    cmp byte [ds:0x500], 0x22   ; must be 0x22 (aliased)
    jne fail_7

    ; ── Step 8: Subfunction 01h — Re-enable A20 ─────────────────────────────
    mov ax, 0x2401
    int 0x15
    jc  fail_8
    cmp ah, 0x00
    jne fail_8

    ; ── Step 9: A20 on again — writes must be distinct ──────────────────────
    mov byte [ds:0x500], 0x33
    mov byte [es:0x510], 0x44   ; goes to linear 0x100500 now
    cmp byte [ds:0x500], 0x33   ; must still be 0x33
    jne fail_9

    ; All tests passed
    mov ax, 0x4C00
    int 0x21

fail_1:
    mov ax, 0x4C01
    int 0x21
fail_2:
    mov ax, 0x4C02
    int 0x21
fail_3:
    mov ax, 0x4C03
    int 0x21
fail_4:
    mov ax, 0x4C04
    int 0x21
fail_5:
    mov ax, 0x4C05
    int 0x21
fail_6:
    mov ax, 0x4C06
    int 0x21
fail_7:
    mov ax, 0x4C07
    int 0x21
fail_8:
    mov ax, 0x4C08
    int 0x21
fail_9:
    mov ax, 0x4C09
    int 0x21
