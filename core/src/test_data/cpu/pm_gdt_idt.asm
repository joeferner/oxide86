[CPU 286]
[ORG 0x100]

; Protected Mode Step 2: LGDT / LIDT / SGDT / SIDT
;
; Tests:
;   1. LGDT loads a GDT descriptor (limit + 24-bit base) from memory
;   2. SGDT stores the GDTR back; compare with the original
;   3. LIDT loads an IDT descriptor from memory
;   4. SIDT stores the IDTR back; compare with the original
;   5. SGDT/SIDT to a different buffer still matches
;
; 286 descriptor table register format (6 bytes):
;   bytes 0-1: limit (16-bit)
;   bytes 2-4: base address (24-bit, little-endian)
;   byte  5:   reserved (undefined on 286 SGDT/SIDT)

section .text
start:
    mov si, msg_banner
    call print_string

    mov word [pass_count], 0
    mov word [fail_count], 0

    call test_lgdt_sgdt
    call test_lidt_sidt
    call test_sgdt_second_buffer
    call test_sidt_second_buffer

    call print_summary

    mov ah, 4Ch
    mov al, [fail_count]
    int 21h

;=============================================================================
; Test: LGDT then SGDT — load GDT register, store it back, compare
;=============================================================================
test_lgdt_sgdt:
    mov si, test_lgdt_sgdt_name
    call print_test_name

    ; Load GDTR from our test descriptor
    lgdt [test_gdtr]

    ; Store GDTR to output buffer
    sgdt [sgdt_buf]

    ; Compare limit (bytes 0-1)
    mov ax, [test_gdtr]
    mov bx, [sgdt_buf]
    cmp ax, bx
    jne .fail_limit

    ; Compare base byte 0 (byte 2)
    mov al, [test_gdtr + 2]
    mov bl, [sgdt_buf + 2]
    cmp al, bl
    jne .fail_base

    ; Compare base byte 1 (byte 3)
    mov al, [test_gdtr + 3]
    mov bl, [sgdt_buf + 3]
    cmp al, bl
    jne .fail_base

    ; Compare base byte 2 (byte 4)
    mov al, [test_gdtr + 4]
    mov bl, [sgdt_buf + 4]
    cmp al, bl
    jne .fail_base

    call print_pass
    ret

.fail_limit:
    mov si, msg_lgdt_sgdt_limit_fail
    call print_fail
    ret

.fail_base:
    mov si, msg_lgdt_sgdt_base_fail
    call print_fail
    ret

;=============================================================================
; Test: LIDT then SIDT — load IDT register, store it back, compare
;=============================================================================
test_lidt_sidt:
    mov si, test_lidt_sidt_name
    call print_test_name

    ; Load IDTR from our test descriptor
    lidt [test_idtr]

    ; Store IDTR to output buffer
    sidt [sidt_buf]

    ; Compare limit (bytes 0-1)
    mov ax, [test_idtr]
    mov bx, [sidt_buf]
    cmp ax, bx
    jne .fail_limit

    ; Compare base byte 0 (byte 2)
    mov al, [test_idtr + 2]
    mov bl, [sidt_buf + 2]
    cmp al, bl
    jne .fail_base

    ; Compare base byte 1 (byte 3)
    mov al, [test_idtr + 3]
    mov bl, [sidt_buf + 3]
    cmp al, bl
    jne .fail_base

    ; Compare base byte 2 (byte 4)
    mov al, [test_idtr + 4]
    mov bl, [sidt_buf + 4]
    cmp al, bl
    jne .fail_base

    call print_pass
    ret

.fail_limit:
    mov si, msg_lidt_sidt_limit_fail
    call print_fail
    ret

.fail_base:
    mov si, msg_lidt_sidt_base_fail
    call print_fail
    ret

;=============================================================================
; Test: SGDT to a second buffer — verify it still matches
; (catches bugs where SGDT only works once or corrupts state)
;=============================================================================
test_sgdt_second_buffer:
    mov si, test_sgdt_second_name
    call print_test_name

    ; GDTR should still hold what we loaded in test_lgdt_sgdt
    sgdt [sgdt_buf2]

    ; Compare with the original test_gdtr values
    mov ax, [test_gdtr]
    mov bx, [sgdt_buf2]
    cmp ax, bx
    jne .fail

    mov al, [test_gdtr + 2]
    mov bl, [sgdt_buf2 + 2]
    cmp al, bl
    jne .fail

    mov al, [test_gdtr + 3]
    mov bl, [sgdt_buf2 + 3]
    cmp al, bl
    jne .fail

    mov al, [test_gdtr + 4]
    mov bl, [sgdt_buf2 + 4]
    cmp al, bl
    jne .fail

    call print_pass
    ret

.fail:
    mov si, msg_sgdt_second_fail
    call print_fail
    ret

;=============================================================================
; Test: SIDT to a second buffer — verify it still matches
;=============================================================================
test_sidt_second_buffer:
    mov si, test_sidt_second_name
    call print_test_name

    ; IDTR should still hold what we loaded in test_lidt_sidt
    sidt [sidt_buf2]

    ; Compare with the original test_idtr values
    mov ax, [test_idtr]
    mov bx, [sidt_buf2]
    cmp ax, bx
    jne .fail

    mov al, [test_idtr + 2]
    mov bl, [sidt_buf2 + 2]
    cmp al, bl
    jne .fail

    mov al, [test_idtr + 3]
    mov bl, [sidt_buf2 + 3]
    cmp al, bl
    jne .fail

    mov al, [test_idtr + 4]
    mov bl, [sidt_buf2 + 4]
    cmp al, bl
    jne .fail

    call print_pass
    ret

.fail:
    mov si, msg_sidt_second_fail
    call print_fail
    ret

;=============================================================================
; Helpers (same pattern as op286.asm)
;=============================================================================

print_test_name:
    call print_string
    mov al, ':'
    call print_char
    mov al, ' '
    call print_char
    ret

print_pass:
    mov si, msg_pass
    call print_string
    inc word [pass_count]
    ret

print_fail:
    push si
    mov si, msg_fail
    call print_string
    pop si
    call print_string
    call print_newline
    inc word [fail_count]
    ret

print_summary:
    call print_newline
    mov si, msg_summary
    call print_string
    mov ax, [pass_count]
    call print_number
    mov si, msg_passed
    call print_string
    mov ax, [fail_count]
    call print_number
    mov si, msg_failed
    call print_string
    call print_newline
    ret

print_string:
    push ax
    push dx
.loop:
    lodsb
    test al, al
    jz .done
    mov dl, al
    mov ah, 02h
    int 21h
    jmp .loop
.done:
    pop dx
    pop ax
    ret

print_char:
    push ax
    push dx
    mov dl, al
    mov ah, 02h
    int 21h
    pop dx
    pop ax
    ret

print_newline:
    push ax
    mov al, 13
    call print_char
    mov al, 10
    call print_char
    pop ax
    ret

print_number:
    push ax
    push bx
    push cx
    push dx

    mov cx, 0
    mov bx, 10

    test ax, ax
    jnz .convert
    mov al, '0'
    call print_char
    jmp .done

.convert:
.divide:
    xor dx, dx
    div bx
    push dx
    inc cx
    test ax, ax
    jnz .divide

.print:
    pop ax
    add al, '0'
    call print_char
    loop .print

.done:
    pop dx
    pop cx
    pop bx
    pop ax
    ret

;=============================================================================
; Data
;=============================================================================
section .data

msg_banner: db '=== 286 Protected Mode Step 2: LGDT/LIDT/SGDT/SIDT ===', 13, 10, 0

test_lgdt_sgdt_name:    db 'LGDT/SGDT', 0
test_lidt_sidt_name:    db 'LIDT/SIDT', 0
test_sgdt_second_name:  db 'SGDT 2nd buf', 0
test_sidt_second_name:  db 'SIDT 2nd buf', 0

msg_pass: db 'PASS', 13, 10, 0
msg_fail: db 'FAIL - ', 0

msg_lgdt_sgdt_limit_fail:   db 'SGDT limit does not match LGDT input', 0
msg_lgdt_sgdt_base_fail:    db 'SGDT base does not match LGDT input', 0
msg_lidt_sidt_limit_fail:   db 'SIDT limit does not match LIDT input', 0
msg_lidt_sidt_base_fail:    db 'SIDT base does not match LIDT input', 0
msg_sgdt_second_fail:       db 'SGDT to 2nd buffer does not match original', 0
msg_sidt_second_fail:       db 'SIDT to 2nd buffer does not match original', 0

msg_summary: db '--- Summary ---', 13, 10, 0
msg_passed:  db ' passed, ', 0
msg_failed:  db ' failed', 0

; Test GDT descriptor: limit=0x0017 (3 entries * 8 - 1), base=0x012345
test_gdtr:
    dw 0x0017           ; limit
    db 0x45, 0x23, 0x01 ; base (24-bit little-endian: 0x012345)
    db 0x00             ; reserved

; Test IDT descriptor: limit=0x07FF (256 entries * 8 - 1), base=0x0ABCDE
test_idtr:
    dw 0x07FF           ; limit
    db 0xDE, 0xBC, 0x0A ; base (24-bit little-endian: 0x0ABCDE)
    db 0x00             ; reserved

section .bss
pass_count:  resw 1
fail_count:  resw 1
sgdt_buf:    resb 6
sgdt_buf2:   resb 6
sidt_buf:    resb 6
sidt_buf2:   resb 6
