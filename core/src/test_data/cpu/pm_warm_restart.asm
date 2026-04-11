[CPU 286]
[ORG 0x100]

; Test: CMOS[0x0F] = 0x0A warm-restart behavior
;
; Verifies that when a program:
;   1. Writes "BEFORE RESET" to the screen via INT 21h and positions cursor to row 1
;   2. Writes a re-entry CS:IP to physical [40:67] (BDA warm-boot vector)
;   3. Writes 0x0A to CMOS[0x0F] (shutdown status = JMP to [40:67] after reset)
;   4. Writes 0xFE to port 0x64 (keyboard controller pulse reset)
; then Computer::reset() resumes at the address stored in [40:67] rather than
; restarting from the program entry point, AND the screen content written before
; the reset is preserved (video card VRAM is not cleared on warm restart).
;
; The cursor position is explicitly set to row 1, col 0 before printing "AFTER RESET"
; because the BDA cursor registers are re-initialized on warm restart (correct behavior).
;
; Expected screen: "BEFORE RESET" on row 0, "AFTER RESET" on row 1.
; Expected exit code: 0x00 (success)

%macro print_str 1
    mov ah, 0x09
    mov dx, %1
    int 0x21
%endmacro

start:
    ; Read CMOS[0x0F]: 0x0A means this is the post-restart run.
    mov al, 0x0F
    out 0x70, al
    in al, 0x71
    cmp al, 0x0A
    je post_restart

    ; === First run ===

    ; Print "BEFORE RESET" to the screen (cursor starts at row 0, col 0)
    print_str msg_before

    ; Write warm-boot re-entry CS:IP to BDA [40:67]/[40:69].
    ; After reset, Computer::reset() will read this and start there
    ; instead of the normal program entry point.
    mov ax, 0x0040
    mov es, ax
    mov word [es:0x0067], post_restart  ; IP = offset of post_restart label
    mov [es:0x0069], cs                 ; CS = current program segment

    ; Write CMOS[0x0F] = 0x0A:
    ;   shutdown status 0x0A = "JMP to dword at [40:67] after reset"
    mov al, 0x0F
    out 0x70, al
    mov al, 0x0A
    out 0x71, al

    ; Trigger CPU reset via keyboard controller (8042 command 0xFE)
    mov al, 0xFE
    out 0x64, al

    ; Should not reach here — reset should redirect to post_restart
    hlt
    mov ax, 0x4C01  ; exit 1 = reset did not trigger
    int 0x21

post_restart:
    ; === Post warm-restart ===
    ; Execution resumes here because [40:67] = this address.
    ; Verify CMOS[0x0F] is still 0x0A (confirms we came via warm restart).
    mov al, 0x0F
    out 0x70, al
    in al, 0x71
    cmp al, 0x0A
    jne fail

    ; Clear CMOS[0x0F] so this doesn't loop on a hypothetical second restart
    mov al, 0x0F
    out 0x70, al
    xor al, al
    out 0x71, al

    ; Set cursor to row 1, col 0 (BDA cursor position is reset on warm restart;
    ; we must position explicitly so "AFTER RESET" appears below "BEFORE RESET")
    mov ah, 0x02    ; INT 10h AH=02h: set cursor position
    xor bh, bh      ; page 0
    mov dh, 1       ; row 1
    xor dl, dl      ; col 0
    int 0x10

    ; Print "AFTER RESET" on row 1 — VRAM on row 0 must still have "BEFORE RESET"
    print_str msg_after

    ; Exit success
    mov ax, 0x4C00
    int 0x21

fail:
    mov ax, 0x4C02  ; exit 2 = CMOS[0x0F] was not 0x0A on re-entry
    int 0x21

msg_before  db 'BEFORE RESET', 0x0D, 0x0A, '$'
msg_after   db 'AFTER RESET', 0x0D, 0x0A, '$'
