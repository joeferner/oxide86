; Keyboard test: custom INT 09h reads port 0x60, then chains to BIOS.
; Verifies ALT+F is correctly reported as scan=0x21, ascii=0x00.
;
; Regression test for: when a custom INT 09h handler reads port 0x60
; directly (clearing OBF), then chains to the old BIOS INT 09h handler,
; process_key_presses must not load the next queued scan code between those
; two events. Before the fix, the chained BIOS handler would read the next
; key (0x21/'F') instead of 0x38 (ALT), so the BDA ALT flag was never set
; and Alt+F arrived as plain 'F' (ascii=0x66) instead of an extended key
; (ascii=0x00).
;
; The fix gates process_key_presses on the keyboard IRQ not being in service,
; keeping the next key out of the controller until the BIOS handler has
; finished processing the current scan code.
;
; Test input:  push_key_press: 0x38 (ALT press), 0x21 (F press), 0xA1 (F release)
; Expected:    INT 16h AH=00h returns AH=0x21, AL=0x00

[CPU 8086]
org 0x0100          ; .COM file

start:
    ; Save old INT 09h vector from IVT[9] (physical 0x0024)
    xor  ax, ax
    mov  es, ax
    mov  ax, [es:0x0024]
    mov  [old_int09_off], ax
    mov  ax, [es:0x0026]
    mov  [old_int09_seg], ax

    ; Install custom INT 09h handler
    mov  word [es:0x0024], int09_handler
    mov  [es:0x0026], cs

    sti                     ; Enable hardware interrupts (allow IRQ1)

    ; Blocking read — run() pauses here after handler is installed.
    ; The test then pushes Alt+F scan codes and calls run() again.
wait_key:
    mov  ah, 0x00           ; INT 16h AH=00h: blocking read
    int  0x16               ; AH=scan code, AL=ASCII

    ; Expect Alt+F: scan=0x21, ascii=0x00 (ALT combo => ascii=0x00)
    cmp  ah, 0x21
    jne  wait_key
    cmp  al, 0x00
    jne  wait_key

    mov  ah, 0x4C
    xor  al, al
    int  0x21               ; Exit 0 (success)

fail:
    mov  ah, 0x4C
    mov  al, 0x01
    int  0x21               ; Exit 1 (fail)


; -----------------------------------------------------------------------------
; Custom INT 09h handler
;
; Reads scan code from port 0x60 directly (clears OBF), then chains to the
; old BIOS INT 09h via pushf + call far — the same pattern used by DOSSHELL
; and similar DOS programs.
;
; The chain is done with pushf + call far so the stack looks like an INT
; frame: [IP][CS][FLAGS], which is what patch_flags_and_iret expects.
; After the chained handler returns (via its IRET), execution continues here
; and our own IRET completes the hardware-interrupt return.
; -----------------------------------------------------------------------------
int09_handler:
    push ax
    in   al, 0x60           ; Read scan code from keyboard port (clears OBF)

    pushf                   ; Build INT-like frame: [IP][CS][FLAGS] on stack
    call far [cs:old_int09_off] ; Chain to old BIOS INT 09h

    pop  ax
    iret                    ; Return from hardware IRQ

old_int09_off:  dw 0
old_int09_seg:  dw 0
