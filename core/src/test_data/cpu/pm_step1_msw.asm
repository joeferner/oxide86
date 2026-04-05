[CPU 286]
[ORG 0x100]

; Protected Mode Step 1: CR0/MSW and System Registers
; Tests SMSW and LMSW instructions with actual CR0 state tracking.
;
; What this tests:
;   - SMSW returns the current MSW (CR0 low 16 bits)
;   - Initially MSW should have PE=0 (real mode)
;   - LMSW can set the PE bit (and other bits)
;   - LMSW cannot clear the PE bit once set (286 behavior)
;   - SMSW to register and to memory both work
;
; NOTE: Once PE is set via LMSW on a real 286, the CPU is in protected mode
; and real-mode INT calls will no longer work correctly. We structure the
; tests so that all output happens BEFORE setting PE, and after setting PE
; we only do SMSW checks that don't require INT calls.
;
; The final check results are stored in memory and validated before PE is set
; (for the pre-PE tests) and via HLT-based signaling after PE is set.

section .text
start:
    ; Print banner
    mov si, msg_banner
    call print_string

    ; Initialize test counters
    mov word [pass_count], 0
    mov word [fail_count], 0

    ; === Tests that run before entering protected mode ===
    call test_smsw_initial
    call test_smsw_to_memory
    call test_lmsw_set_bits

    ; === Test that LMSW sets PE and SMSW reads it back ===
    ; After this test PE will be set, so no more INT 21h calls.
    ; We do ALL printing before the LMSW that sets PE.
    call test_lmsw_set_pe

    ; Print summary
    call print_summary

    ; Exit: use fail_count as exit code so Rust test catches failures
    mov ah, 4Ch
    mov al, [fail_count]
    int 21h

;=============================================================================
; Test: SMSW initial value should have PE=0
; On a fresh 286 in real mode, MSW should be 0x0000 (or at least PE=0)
;=============================================================================
test_smsw_initial:
    mov si, test_smsw_initial_name
    call print_test_name

    ; SMSW to AX — opcode: 0F 01 /4 (mod=11, rm=AX)
    smsw ax

    ; Check PE bit (bit 0) is clear
    test ax, 1
    jnz .fail_pe

    call print_pass
    ret

.fail_pe:
    mov si, msg_smsw_initial_pe_fail
    call print_fail
    ret

;=============================================================================
; Test: SMSW to memory location
; Verifies SMSW works with a memory operand, not just registers
;=============================================================================
test_smsw_to_memory:
    mov si, test_smsw_to_memory_name
    call print_test_name

    ; Clear the target first
    mov word [msw_buffer], 0xFFFF

    ; SMSW to memory — opcode: 0F 01 /4 (mod!=11)
    smsw [msw_buffer]

    ; Check the stored value has PE=0
    mov ax, [msw_buffer]
    test ax, 1
    jnz .fail

    call print_pass
    ret

.fail:
    mov si, msg_smsw_to_memory_fail
    call print_fail
    ret

;=============================================================================
; Test: LMSW can set MP, EM, TS bits (bits 1-3) and SMSW reads them back
; We set bits 1-3 without setting PE so we stay in real mode.
;=============================================================================
test_lmsw_set_bits:
    mov si, test_lmsw_set_bits_name
    call print_test_name

    ; Set MP=1, EM=1, TS=1 (bits 1,2,3) but PE=0 (bit 0)
    mov ax, 0x000E    ; bits 1,2,3 set
    lmsw ax

    ; Read back
    smsw ax

    ; Check bits 1-3 are set
    and ax, 0x000E
    cmp ax, 0x000E
    jne .fail

    ; Clear the bits back (LMSW can clear bits 1-3, just not bit 0 once set)
    mov ax, 0x0000
    lmsw ax

    call print_pass
    ret

.fail:
    mov si, msg_lmsw_set_bits_fail
    call print_fail

    ; Try to clear bits anyway for subsequent tests
    mov ax, 0x0000
    lmsw ax
    ret

;=============================================================================
; Test: LMSW sets PE bit and SMSW reads it back
; After this, the CPU is in protected mode. We do all printing BEFORE
; the LMSW that sets PE.
;
; Strategy: we know the test passes if SMSW returns PE=1 after LMSW.
; We print the test name, then do the LMSW+SMSW, then print result.
; Since we're not setting up GDT/IDT, we can't safely do much in PM.
; But SMSW and basic register ops don't need segment loads.
;
; IMPORTANT: On a real 286, setting PE without a GDT would crash on
; the next segment load. The emulator should track the PE bit in CR0
; and return it via SMSW, even if full PM isn't implemented yet.
;=============================================================================
test_lmsw_set_pe:
    mov si, test_lmsw_set_pe_name
    call print_test_name

    ; Set PE bit
    mov ax, 0x0001
    lmsw ax

    ; Read back MSW
    smsw ax

    ; Check PE bit is set — but we can't print via INT 21h anymore
    ; if we're truly in protected mode. However, for Step 1 of the
    ; implementation, the emulator just tracks CR0 without actually
    ; switching addressing mode, so INT 21h still works.
    test ax, 1
    jz .fail

    ; Also verify SMSW to memory works with PE set
    smsw [msw_buffer]
    mov bx, [msw_buffer]
    test bx, 1
    jz .fail

    ; Now test that LMSW cannot CLEAR PE once set (286 behavior)
    mov ax, 0x0000    ; Try to clear everything including PE
    lmsw ax
    smsw ax
    test ax, 1        ; PE should still be 1
    jz .fail_cant_clear

    call print_pass
    ret

.fail:
    mov si, msg_lmsw_set_pe_fail
    call print_fail
    ret

.fail_cant_clear:
    mov si, msg_lmsw_cant_clear_pe_fail
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

msg_banner: db '=== 286 Protected Mode Step 1: MSW Tests ===', 13, 10, 0

test_smsw_initial_name:     db 'SMSW initial PE=0', 0
test_smsw_to_memory_name:   db 'SMSW to memory', 0
test_lmsw_set_bits_name:    db 'LMSW set MP/EM/TS', 0
test_lmsw_set_pe_name:      db 'LMSW set PE + cant clear', 0

msg_pass: db 'PASS', 13, 10, 0
msg_fail: db 'FAIL - ', 0

msg_smsw_initial_pe_fail:       db 'SMSW should return PE=0 initially', 0
msg_smsw_to_memory_fail:        db 'SMSW to memory should have PE=0', 0
msg_lmsw_set_bits_fail:         db 'LMSW should set MP/EM/TS bits', 0
msg_lmsw_set_pe_fail:           db 'SMSW should return PE=1 after LMSW', 0
msg_lmsw_cant_clear_pe_fail:    db 'LMSW should not be able to clear PE on 286', 0

msg_summary: db '--- Summary ---', 13, 10, 0
msg_passed:  db ' passed, ', 0
msg_failed:  db ' failed', 0

section .bss
pass_count:  resw 1
fail_count:  resw 1
msw_buffer:  resw 1
