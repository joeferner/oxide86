; Keyboard test: custom INT 09h handler mimicking the IO.SYS keyboard intercept pattern
;
; This test verifies:
;   1. A custom INT 09h handler can replace the BIOS handler via the IVT
;   2. INT 15h AH=4Fh returns CF=1 (key NOT intercepted -> pass to BDA buffer)
;   3. The BDA keyboard buffer stores entries as [ascii_code][scan_code]
;      (ascii at the lower address, scan at the upper address)
;
; Findings documented in: ai-analysis/msdos4-keyboard-interrupt-handling.md
;
; The handler mirrors the IO.SYS INT 09h pattern observed in MS-DOS 4.01:
;   - Read scan code from port 0x60 (clears OBF)
;   - Skip break codes (bit 7 set = key release)
;   - Call INT 15h AH=4Fh (keyboard intercept) with CF=1 (calling convention)
;   - CF=1 on return: key NOT intercepted -> write [ascii][scan] to BDA ring buffer
;   - CF=0 on return: key intercepted/consumed -> discard key
;   - Send Non-Specific EOI to PIC (port 0x20 <- 0x20)

[CPU 8086]
org 0x0100          ; .COM files start at CS:0100h

start:
    ; Install our custom INT 09h handler into IVT[9]
    ; IVT entry 9 is at physical 0x0024 (9 * 4 = 0x24)
    xor  ax, ax
    mov  es, ax
    mov  word [es:0x0024], int09_handler   ; offset
    mov  [es:0x0026], cs                   ; segment

    sti                 ; Enable hardware interrupts so IRQ1 (keyboard) can fire

poll:
    mov  ah, 0x01       ; INT 16h AH=01h: peek keyboard buffer (ZF=1: empty, ZF=0: key ready)
    int  0x16
    jz   poll           ; Loop until a key is available

    ; Consume the key from the BDA buffer
    mov  ah, 0x00       ; INT 16h AH=00h: read key (AH=scan code, AL=ASCII)
    int  0x16

    ; Verify: Enter key expected (scan=0x1C, ascii=0x0D)
    cmp  ah, 0x1C       ; BIOS scan code for Enter
    jne  fail
    cmp  al, 0x0D       ; ASCII carriage return
    jne  fail

    ; Success
    mov  ah, 0x4C
    mov  al, 0x00
    int  0x21

fail:
    mov  ah, 0x4C
    mov  al, 0x01
    int  0x21


; ─────────────────────────────────────────────────────────────────────────────
; Custom INT 09h handler
;
; Mirrors the IO.SYS pattern documented in:
;   ai-analysis/msdos4-keyboard-interrupt-handling.md
;
; Key ordering in the BDA ring buffer (confirmed from IO.SYS trace and the
; bda_add_key_to_buffer / bda_peek_key fixes in core/src/cpu/bios/bda.rs):
;   byte at tail+0 = ASCII code   (lower address)
;   byte at tail+1 = scan code    (upper address)
;
; BDA ring buffer layout (all offsets relative to DS=0x0040):
;   0x001A  head pointer  (word, offset into BDA)
;   0x001C  tail pointer  (word, offset into BDA)
;   0x001E  buffer start  (16 slots x 2 bytes = 32 bytes)
;   0x003E  buffer end    (one past last valid slot)
; ─────────────────────────────────────────────────────────────────────────────
int09_handler:
    push ax
    push bx
    push si
    push ds

    in   al, 0x60           ; Read scan code from keyboard data port (also clears OBF)

    ; Skip break codes (bit 7 set = key release event)
    test al, 0x80
    jnz  .send_eoi

    mov  bl, al             ; BL = scan code

    ; Translate scan code to ASCII (this test only handles Enter: 0x1C -> 0x0D)
    cmp  bl, 0x1C
    jne  .send_eoi
    mov  bh, 0x0D           ; BH = ASCII 0x0D (carriage return)

    ; ── INT 15h AH=4Fh keyboard intercept ────────────────────────────────────
    ; Calling convention: caller sets CF=1 before INT 15h.
    ; Return:  CF=1 -> key NOT intercepted, buffer it in BDA
    ;          CF=0 -> key consumed by hook, discard it
    ; Our BIOS stub (int15_keyboard_intercept) returns CF=1 (pass-through).
    mov  ah, 0x4F
    stc                     ; Calling convention
    int  0x15
    jnc  .send_eoi          ; CF=0 -> intercepted/consumed -> discard

    ; ── Write key to BDA keyboard ring buffer ────────────────────────────────
    ; Format: [ascii_code at tail][scan_code at tail+1]
    mov  ax, 0x0040
    mov  ds, ax             ; DS = BDA segment

    mov  si, [0x001C]       ; SI = current tail pointer (BDA offset, e.g. 0x001E)

    mov  [si],   bh         ; ascii code at lower address
    mov  [si+1], bl         ; scan code  at upper address

    ; Advance tail (wrap at 0x003E back to 0x001E)
    add  si, 2
    cmp  si, 0x003E
    jb   .store_tail
    mov  si, 0x001E         ; Wrap around to buffer start
.store_tail:
    mov  [0x001C], si       ; Update BDA tail pointer

.send_eoi:
    mov  al, 0x20
    out  0x20, al           ; Non-Specific EOI to PIC

    pop  ds
    pop  si
    pop  bx
    pop  ax
    iret
