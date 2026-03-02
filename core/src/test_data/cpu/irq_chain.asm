; IRQ chaining test: verify that a custom INT 08h handler can chain to the
; original BIOS timer interrupt.
;
; Strategy:
;   1. Save the original INT 08h vector from the IVT.
;   2. Install our own INT 08h handler.
;   3. In our handler: set our_called_flag = 1, then far-jump to the original.
;   4. Enable interrupts and poll INT 1Ah until the BDA tick count changes,
;      which proves the original BIOS handler ran (it updates the BDA counter).
;   5. Verify our flag was set, confirming our handler also ran.
;   6. Exit 0 = success, exit 1 = tick timeout, exit 2 = our handler never fired.

[CPU 8086]
org 0x0100          ; .COM file entry point

start:
    ; --- Save original INT 08h vector from the IVT ---
    ; IVT lives at segment 0; INT 08h entry is at byte offset 8*4 = 0x0020
    xor ax, ax
    mov es, ax
    mov ax, [es:0x0020]         ; original handler offset
    mov [orig_int08_off], ax
    mov ax, [es:0x0022]         ; original handler segment
    mov [orig_int08_seg], ax

    ; --- Install our custom INT 08h handler ---
    mov word [es:0x0020], our_handler
    mov word [es:0x0022], cs

    ; --- Clear the flag before enabling interrupts ---
    mov byte [our_called_flag], 0

    ; --- Enable interrupts so IRQ 0 (INT 08h) can be delivered ---
    sti

    ; --- Read initial BDA tick count via INT 1Ah AH=00h ---
    mov ah, 0x00
    int 0x1a                    ; returns CX:DX = ticks since midnight
    mov bx, dx                  ; BX = initial low word

    mov si, 0xFFFF              ; timeout counter (~9 ticks worth of polling)

poll_loop:
    mov ah, 0x00
    int 0x1a                    ; DX = current low tick word
    cmp dx, bx                  ; has the counter advanced?
    jne check_our_flag          ; yes – original BIOS handler updated BDA
    dec si
    jnz poll_loop

    ; Timeout: tick count never changed – original handler never ran
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

check_our_flag:
    ; Tick count changed – original BIOS INT 08h ran.
    ; Confirm our custom handler also ran by checking the flag.
    cmp byte [our_called_flag], 0x01
    je success

    ; Flag never set – our handler was not called (chaining broken)
    mov ah, 0x4C
    mov al, 0x02
    int 0x21

success:
    ; Both handlers ran: IRQ chaining is working correctly
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

; -------------------------------------------------------
; our_handler – custom INT 08h ISR
;
; The CPU pushes FLAGS, CS, IP when the timer fires and
; jumps here.  We set our flag, then far-jump to the
; original BIOS handler.  The BIOS handler ends with IRET,
; which unwinds those three words and returns to the
; originally-interrupted code.  No extra stack cleanup
; is needed.
; -------------------------------------------------------
our_handler:
    push ax
    mov al, 0x01
    mov [cs:our_called_flag], al    ; record that our handler was called
    pop ax
    jmp far [cs:orig_int08_off]     ; chain: BIOS handler's IRET returns to caller

; -------------------------------------------------------
; Data
; -------------------------------------------------------
our_called_flag db 0
orig_int08_off  dw 0                ; saved original INT 08h offset
orig_int08_seg  dw 0                ; saved original INT 08h segment
