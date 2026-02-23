; pic_isr_reentrance.asm
;
; Tests 8259A PIC ISR re-entrance prevention for INT 09h (keyboard IRQ 1).
;
; On real AT-class hardware the PIC In-Service Register (ISR) keeps IRQ 1
; marked as in-service from when the IRQ fires until the handler writes EOI
; (OUT 0x20, 0x20).  No second keyboard IRQ can be delivered during this
; window, even if the handler re-enables interrupts with STI.
;
; Sierra AGI games (e.g. KQ2 at 10B2:5F6F) execute STI immediately after
; entering their custom INT 09h handler.  Without emulator PIC ISR support,
; if a key-release event is queued alongside a key-press, the release fires
; re-entrantly (inside the press handler, BEFORE port 0x60 is read), which
; overwrites the scan code with the break code.
;
; This test installs a custom INT 09h handler that:
;   1. Detects re-entrance via an "in_handler" counter.
;   2. Immediately executes STI (exactly as game handlers do).
;   3. Reads port 0x60 and logs the scan code.
;   4. Decrements the counter, then chains to the old (BIOS) handler.
;      The BIOS handler sends EOI, clearing the PIC ISR bit.
;
; Expected output with PIC ISR implemented:
;   Reentrant count stays 0 for all keypresses -> PASS
;
; Without PIC ISR (original emulator behavior):
;   Reentrant count may increment when press+release arrive together -> FAIL
;
; Press any key to log it.  ESC to exit.
;
; Build:
;   nasm -f bin pic_isr_reentrance.asm -o pic_isr_reentrance.com
; Run:
;   cargo run -p oxide86-native-gui -- test-programs/keyboard/pic_isr_reentrance.com

[CPU 8086]
org 0x100

start:
    ; Print banner
    mov ah, 0x09
    mov dx, banner
    int 0x21

    ; Save old INT 09h vector (ES:BX on return)
    mov ax, 0x3509
    int 0x21
    mov [old09_off], bx
    mov [old09_seg], es

    ; Install our custom INT 09h handler
    push ds
    push cs
    pop ds                  ; DS = CS (needed for INT 21h AH=25h)
    mov ax, 0x2509
    mov dx, handler09
    int 0x21
    pop ds

main_loop:
    ; Poll key_ready flag (set by handler on each key press)
    cmp byte [key_ready], 0
    je .check_esc

    mov byte [key_ready], 0

    ; Print "Scan:XX"
    mov ah, 0x09
    mov dx, scan_msg
    int 0x21
    mov al, [last_scan]
    call print_byte_hex

    ; Print " Reentrant:NN"
    mov ah, 0x09
    mov dx, reent_msg
    int 0x21
    mov al, [reentrant_count]
    call print_byte_hex

    ; Print PASS or FAIL tag for this event
    cmp byte [reentrant_count], 0
    jne .line_fail
    mov ah, 0x09
    mov dx, pass_tag
    int 0x21
    jmp .line_done
.line_fail:
    mov ah, 0x09
    mov dx, fail_tag
    int 0x21
.line_done:
    mov ah, 0x09
    mov dx, crlf
    int 0x21

.check_esc:
    cmp byte [last_scan], 0x01   ; scan code 0x01 = ESC press
    je exit_prog

    ; Delay to avoid busy-spinning too aggressively
    mov cx, 0xFFFF
.delay:
    loop .delay
    jmp main_loop

; ---------------------------------------------------------------
exit_prog:
    ; Restore old INT 09h vector
    push ds
    mov ax, [old09_seg]
    mov ds, ax
    mov dx, [old09_off]
    mov ax, 0x2509
    int 0x21
    pop ds

    ; Print final result
    mov ah, 0x09
    mov dx, final_hdr
    int 0x21

    cmp byte [reentrant_count], 0
    jne .overall_fail

    mov ah, 0x09
    mov dx, pass_msg
    int 0x21
    jmp .done

.overall_fail:
    mov ah, 0x09
    mov dx, fail_msg
    int 0x21

.done:
    mov ax, 0x4C00
    int 0x21

; ===============================================================
; Custom INT 09h handler
; ---------------------------------------------------------------
; Mimics a re-entrant game handler by executing STI immediately
; after entry.  The PIC ISR should prevent any nested INT 09h
; delivery until the chained BIOS handler sends EOI.
; ===============================================================
handler09:
    push ax

    ; --- Re-entrance check ---
    ; If in_handler != 0 when we arrive, we are being called
    ; from inside a still-running instance of this handler.
    ; On real hardware (and correct emulators) this never happens.
    cmp byte [in_handler], 0
    je .first_entry
    inc byte [reentrant_count]   ; count the nested call
.first_entry:
    inc byte [in_handler]

    ; *** STI: re-enable interrupts immediately ***
    ; On real hardware the PIC ISR blocks same-level re-entry here.
    ; Without PIC ISR emulation a queued release fires at this point.
    sti

    ; Read scan code from keyboard controller port 0x60
    in al, 0x60
    mov [last_raw], al

    ; Record press events (bit 7 clear = press, bit 7 set = release)
    test al, 0x80
    jnz .not_press
    mov [last_scan], al
    mov byte [key_ready], 1
.not_press:

    dec byte [in_handler]
    pop ax

    ; Chain to old (BIOS) INT 09h handler.
    ; The BIOS handler buffers the key and sends EOI, which clears
    ; the PIC ISR bit 1 and allows the next keyboard IRQ to fire.
    pushf
    call far [cs:old09_off]
    iret

; ===============================================================
; Helpers
; ===============================================================

; Print AL as two uppercase hex digits
print_byte_hex:
    push ax
    push cx
    mov cl, 4
    shr al, cl              ; high nibble
    call .nibble
    pop cx
    pop ax
    push ax
    and al, 0x0F            ; low nibble
    call .nibble
    pop ax
    ret
.nibble:
    push dx
    cmp al, 9
    jbe .is_num
    add al, 'A' - 10
    jmp .emit
.is_num:
    add al, '0'
.emit:
    mov dl, al
    mov ah, 0x02
    int 0x21
    pop dx
    ret

; ===============================================================
; Data
; ===============================================================
in_handler:      db 0     ; nesting depth (0 = idle, 1 = in handler)
reentrant_count: db 0     ; number of re-entrant INT 09h calls observed
last_scan:       db 0     ; scan code of most recent key press
last_raw:        db 0     ; raw value from port 0x60 (press or release)
key_ready:       db 0     ; set to 1 when a press has been logged

old09_off:       dw 0     ; saved INT 09h vector
old09_seg:       dw 0

banner:
    db '=== PIC ISR Re-entrance Test ===', 13, 10
    db 'Custom INT 09h handler immediately does STI,', 13, 10
    db 'mimicking Sierra AGI / KQ2 game handlers.', 13, 10
    db 'PASS = reentrant count stays 0 (PIC ISR working).', 13, 10
    db 'FAIL = reentrant count > 0 (no PIC ISR protection).', 13, 10
    db 'Press keys. ESC to quit.', 13, 10, 13, 10, '$'

scan_msg:  db 'Scan:', '$'
reent_msg: db ' Reentrant:', '$'
pass_tag:  db ' [PASS]', '$'
fail_tag:  db ' [FAIL]', '$'
crlf:      db 13, 10, '$'

final_hdr:
    db 13, 10, '--- Final Result ---', 13, 10, '$'
pass_msg:
    db 'PASS: No re-entrant INT 09h calls detected.', 13, 10
    db 'PIC ISR (In-Service Register) working correctly.', 13, 10, '$'
fail_msg:
    db 'FAIL: Re-entrant INT 09h calls were detected!', 13, 10
    db 'PIC ISR not implemented or not working.', 13, 10, '$'
