; INT 1Ah Function 00h - Get Tick Count, and Function 01h - Set Tick Count
; Expected time: 3/2/2026 11:05 AM
; At 11:05:00 AM: ticks from midnight = (11*3600 + 5*60 + 0) * 18.2065 ≈ 724,490
; At 11:05:59 AM: ≈ 725,563
; We use a loose window: 724,000 (0x000B_0B98) to 726,000 (0x000B_12F0)
; Tick count is returned/set as CX:DX (CX=high word, DX=low word)

[CPU 8086]
org 0x0100

start:
    ; --- Part 1: Read and validate the current tick count (Function 00h) ---
    mov ah, 0x00        ; Function 00h: read tick count
    int 0x1A            ; CX = high word, DX = low word, AL = midnight flag

    ; Check midnight rollover flag - should be 0
    cmp al, 0x00
    jne fail

    ; Check CX (high word) = 0x000B
    cmp cx, 0x000B
    jne fail

    ; Check DX (low word) is in range [0x0B98, 0x12F0]
    cmp dx, 0x0B98
    jb  fail
    cmp dx, 0x12F0
    ja  fail

    ; --- Part 2: Set a known tick count (Function 01h) ---
    ; Write a known sentinel value: 0x000B_0F00 (~724,992 ticks, ~39,834 seconds ~= 11:03:54)
    ; This is within working hours so midnight flag should remain 0
    mov ah, 0x01        ; Function 01h: set tick count
    mov cx, 0x000B      ; high word
    mov dx, 0x0F00      ; low word
    int 0x1A            ; sets the tick counter to CX:DX, clears midnight flag

    ; --- Part 3: Read back and verify the value was stored (Function 00h) ---
    mov ah, 0x00        ; Function 00h: read tick count again
    int 0x1A

    ; Midnight flag should still be 0 after our set
    cmp al, 0x00
    jne fail

    ; High word must still be 0x000B
    cmp cx, 0x000B
    jne fail

    ; Low word must be >= 0x0F00 (time is always advancing, never goes backward)
    cmp dx, 0x0F00
    jb  fail

    ; Low word should not have advanced more than ~110 ticks (~6 seconds) since we set it
    ; 0x0F00 + 0x006E = 0x0F6E
    cmp dx, 0x0F6E
    ja  fail

    ; Success
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21
