; cpu_detect.asm - CPU detection and flag behaviour tests
;
; Implements the same two-stage logic used by real BIOS detection routines
; (e.g. SvardOS / DR-DOS Verify386):
;
;   Stage 1 – PUSH SP quirk
;     On 8086/88 the processor decrements SP *before* pushing, so the value
;     written to the stack is the already-decremented SP.  286+ push the
;     original (pre-decrement) value.
;
;   Stage 2 – IOPL bits in FLAGS  (only reached on 286+)
;     On 286 real mode bits 12-15 of FLAGS are always 0; POPF cannot change
;     them.  On 386+ real mode POPF can freely set IOPL (bits 12-13).
;
; Additional tests verify per-CPU flag behaviour via PUSHF/POPF:
;   • CF round-trip (all CPUs)
;   • ZF round-trip (all CPUs)
;   • CF/OF multi-flag round-trip (all CPUs)
;   • After POPF 0x0000: bits 12-15 must be 0xF000 on 8086, 0x0000 on 286+
;
; Exit codes:
;   0x00 = detected 8086  (SP quirk present, bits 12-15 confirmed 0xF000)
;   0x01 = detected 286   (no SP quirk, IOPL not settable, bits 12-15 = 0x0000)
;   0x02 = detected 386+  (no SP quirk, IOPL settable)
;   0xFF = unexpected flag behaviour for the detected CPU type

[CPU 8086]
[ORG 0x100]

section .text
start:
    mov word [pass_count], 0
    mov word [fail_count], 0

    mov si, msg_banner
    call print_string

    ; ---------------------------------------------------------------
    ; Common flag round-trip tests (must pass on every CPU)
    ; ---------------------------------------------------------------
    call test_cf_roundtrip
    call test_zf_roundtrip
    call test_cf_of_multi

    cmp word [fail_count], 0
    jne .common_fail

    ; ---------------------------------------------------------------
    ; Stage 1: PUSH SP quirk  (8086 vs 286+)
    ; On 8086:  push sp writes the already-decremented SP value,
    ;           so ax != sp after the round-trip.
    ; On 286+:  push sp writes the original SP, so ax == sp.
    ; ---------------------------------------------------------------
    push sp
    pop ax
    cmp ax, sp
    jne .is_8086

    ; ---------------------------------------------------------------
    ; Stage 2: IOPL bits in FLAGS  (286 vs 386+)
    ; Try to set bits 12-13 (IOPL=3) via POPF.
    ; On 286 real mode: POPF cannot set these bits (remain 0).
    ; On 386+ real mode: POPF can freely set IOPL bits.
    ; ---------------------------------------------------------------
    mov ax, 0x3000
    push ax
    popf
    pushf
    pop bx
    and bx, 0x3000
    jnz .is_386

    ; ---------------------------------------------------------------
    ; Detected: 286
    ; Validate: after POPF 0x0000 bits 12-15 must be 0x0000
    ; ---------------------------------------------------------------
.is_286:
    call test_high_bits_zero
    cmp word [fail_count], 0
    jne .unexpected_fail

    mov si, msg_286
    call print_string
    call print_summary
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

    ; ---------------------------------------------------------------
    ; Detected: 386+
    ; ---------------------------------------------------------------
.is_386:
    mov si, msg_386
    call print_string
    call print_summary
    mov ah, 0x4C
    mov al, 0x02
    int 0x21

    ; ---------------------------------------------------------------
    ; Detected: 8086
    ; Validate: after POPF 0x0000 bits 12-15 must be 0xF000
    ; ---------------------------------------------------------------
.is_8086:
    call test_high_bits_f000
    cmp word [fail_count], 0
    jne .unexpected_fail

    mov si, msg_8086
    call print_string
    call print_summary
    mov ah, 0x4C
    mov al, 0x00
    int 0x21

.common_fail:
    mov si, msg_common_fail
    call print_string
    call print_summary
    mov ah, 0x4C
    mov al, 0xFF
    int 0x21

.unexpected_fail:
    mov si, msg_unexpected_fail
    call print_string
    call print_summary
    mov ah, 0x4C
    mov al, 0xFF
    int 0x21

;=============================================================================
; Test: CF round-trip through PUSHF/POPF
;=============================================================================
test_cf_roundtrip:
    mov si, name_cf_roundtrip
    call print_test_name

    ; Set CF, save via PUSHF, clear CF, restore via POPF, verify CF restored
    stc
    pushf
    clc
    popf
    jnc .fail           ; CF must be 1

    ; Clear CF, save via PUSHF, set CF, restore via POPF, verify CF cleared
    clc
    pushf
    stc
    popf
    jc .fail            ; CF must be 0

    call print_pass
    ret
.fail:
    mov si, msg_cf_fail
    call print_fail
    ret

;=============================================================================
; Test: ZF round-trip through PUSHF/POPF
;=============================================================================
test_zf_roundtrip:
    mov si, name_zf_roundtrip
    call print_test_name

    ; ZF=1 via OR ax,ax with ax=0, save, then clear ZF, restore, check ZF=1
    xor ax, ax          ; ZF = 1
    pushf
    mov ax, 1
    or ax, ax           ; ZF = 0
    popf
    jnz .fail           ; ZF must be 1 (jnz jumps when ZF=0)

    ; ZF=0 via OR ax,ax with ax=1, save, then set ZF, restore, check ZF=0
    mov ax, 1
    or ax, ax           ; ZF = 0
    pushf
    xor ax, ax          ; ZF = 1
    popf
    jz .fail            ; ZF must be 0 (jz jumps when ZF=1)

    call print_pass
    ret
.fail:
    mov si, msg_zf_fail
    call print_fail
    ret

;=============================================================================
; Test: CF and OF together through PUSHF/POPF, plus reading flags from stack
;
; No extra pop on the .fail paths: pushf is always balanced by popf or pop ax
; before any conditional jump, so the stack is clean at .fail.
;=============================================================================
test_cf_of_multi:
    mov si, name_cf_of_multi
    call print_test_name

    ; Produce OF=1 (signed overflow: 0x7FFF + 1 = 0x8000)
    ; then explicitly set CF=1.  Save both, clear both, restore, verify.
    mov ax, 0x7FFF
    add ax, 1           ; OF = 1, CF = 0  (no unsigned carry)
    stc                 ; CF = 1
    pushf               ; save CF=1, OF=1
    clc                 ; clear CF
    mov ax, 0           ; clear OF (trivially zero)
    popf                ; restore: CF=1, OF=1
    jnc .fail           ; CF must be 1
    jno .fail           ; OF must be 1

    ; Read CF directly from the stacked flags word.
    ; push/pop are balanced before the jump so .fail has no extra value.
    stc
    pushf               ; push flags with CF=1
    mov bp, sp
    mov ax, [bp]        ; read stacked word without popping
    pop ax              ; now pop (clean stack)
    test ax, 0x0001     ; bit 0 = CF
    jz .fail            ; CF must be set in saved flags

    call print_pass
    ret
.fail:
    mov si, msg_multi_fail
    call print_fail
    ret

;=============================================================================
; Test: bits 12-15 are 0x0000 after POPF 0x0000  (286 validation)
;=============================================================================
test_high_bits_zero:
    mov si, name_high_zero
    call print_test_name

    xor ax, ax
    push ax
    popf
    pushf
    pop ax
    and ax, 0xF000
    cmp ax, 0x0000
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_high_zero_fail
    call print_fail
    ret

;=============================================================================
; Test: bits 12-15 are 0xF000 after POPF 0x0000  (8086 validation)
; On 8086 those bits are physically pulled high and cannot be cleared.
;=============================================================================
test_high_bits_f000:
    mov si, name_high_f000
    call print_test_name

    xor ax, ax
    push ax
    popf
    pushf
    pop ax
    and ax, 0xF000
    cmp ax, 0xF000
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_high_f000_fail
    call print_fail
    ret

;=============================================================================
; Helpers
;=============================================================================
print_string:
    mov ah, 0x0E
    mov bx, 0x0007
.loop:
    lodsb
    or al, al
    jz .done
    int 0x10
    jmp .loop
.done:
    ret

print_char:
    mov ah, 0x0E
    mov bx, 0x0007
    int 0x10
    ret

print_newline:
    mov al, 13
    call print_char
    mov al, 10
    call print_char
    ret

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
    mov si, msg_summary
    call print_string
    mov ax, [pass_count]
    call print_decimal
    mov si, msg_passed
    call print_string
    mov ax, [fail_count]
    call print_decimal
    mov si, msg_failed
    call print_string
    call print_newline
    ret

; Print AX as decimal (0-65535).
; Pushes digits onto stack, then pops and prints.
print_decimal:
    push bx
    push cx
    push dx
    mov bx, 10
    xor cx, cx
.push_digit:
    xor dx, dx
    div bx              ; AX = AX/10, DX = remainder
    push dx
    inc cx
    or ax, ax
    jnz .push_digit
.pop_digit:
    pop ax
    add al, '0'
    call print_char
    loop .pop_digit
    pop dx
    pop cx
    pop bx
    ret

;=============================================================================
; Data
;=============================================================================
section .data

msg_banner:   db 'CPU Detection Test', 13, 10, 0
msg_8086:     db 'Result: 8086', 13, 10, 0
msg_286:      db 'Result: 286', 13, 10, 0
msg_386:      db 'Result: 386+', 13, 10, 0
msg_common_fail:     db 'FAIL: common flag tests failed', 13, 10, 0
msg_unexpected_fail: db 'FAIL: cpu-specific flag check failed', 13, 10, 0

msg_pass:     db 'PASS', 13, 10, 0
msg_fail:     db 'FAIL: ', 0

msg_summary:  db '--- Summary ---', 13, 10, 0
msg_passed:   db ' passed, ', 0
msg_failed:   db ' failed', 0

name_cf_roundtrip: db 'CF roundtrip', 0
name_zf_roundtrip: db 'ZF roundtrip', 0
name_cf_of_multi:  db 'CF+OF multi', 0
name_high_zero:    db 'bits12-15=0', 0
name_high_f000:    db 'bits12-15=F000', 0

msg_cf_fail:         db 'CF not preserved', 0
msg_zf_fail:         db 'ZF not preserved', 0
msg_multi_fail:      db 'CF/OF check failed', 0
msg_high_zero_fail:  db 'bits 12-15 not zero on 286', 0
msg_high_f000_fail:  db 'bits 12-15 not F000 on 8086', 0

section .bss
pass_count: resw 1
fail_count: resw 1
