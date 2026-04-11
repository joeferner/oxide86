[CPU 286]
[ORG 0x100]

; Protected Mode Step 1: CR0/MSW and System Registers
; Tests SMSW and LMSW instructions with actual CR0 state tracking.
;
; What this tests:
;   - SMSW returns the current MSW (CR0 low 16 bits)
;   - Initially MSW should have PE=0 (real mode)
;   - LMSW can set MP/EM/TS bits and SMSW reads them back
;   - SMSW to register and to memory both work
;   - LMSW can set PE bit
;   - LMSW cannot clear PE bit once set (286 behavior)
;
; The PE test is last. After PE is set, we cannot use INT calls, so
; we do all printing first, then set PE and store pass/fail in a byte.
; The exit code reflects total failures.

section .text
start:
    mov si, msg_banner
    call print_string

    mov word [pass_count], 0
    mov word [fail_count], 0

    ; === Tests that run in real mode (can print freely) ===
    call test_smsw_initial
    call test_smsw_to_memory
    call test_lmsw_set_bits

    ; === PE test: print name, then do silent checks ===
    mov si, test_lmsw_set_pe_name
    call print_test_name

    ; Set PE bit
    mov ax, 0x0001
    lmsw ax

    ; Read back MSW — check PE=1
    smsw ax
    test ax, 1
    jz .pe_fail

    ; SMSW to memory with PE set
    smsw [msw_buffer]
    mov bx, [msw_buffer]
    test bx, 1
    jz .pe_fail

    ; Test LMSW cannot clear PE (286 behavior)
    mov ax, 0x0000
    lmsw ax
    smsw ax
    test ax, 1
    jz .pe_fail_cant_clear

    ; PE test passed — record it directly (can't call print_pass in PM)
    inc word [pass_count]
    jmp .pe_done

.pe_fail:
    inc word [fail_count]
    jmp .pe_done

.pe_fail_cant_clear:
    inc word [fail_count]

.pe_done:
    ; Now we need to exit. PE is set, so INT 21h dispatch will try
    ; to do GDT lookups on IRET. Set up a minimal GDT so the CPU
    ; can return from the BIOS interrupt handler.
    call setup_exit_gdt

    ; Print summary (INT 21h should work now with GDT in place)
    call print_summary

    ; Exit: use fail_count as exit code
    mov ah, 4Ch
    mov al, [fail_count]
    int 21h

;=============================================================================
; Set up a minimal GDT so INT 21h works with PE=1.
; CS and SS/DS are still real-mode values, so we create descriptors
; that map them to the same physical addresses.
;=============================================================================
setup_exit_gdt:
    ; Compute physical base = CS << 4
    mov ax, cs
    mov cl, 4
    shl ax, cl
    mov [cs_base_lo], ax
    mov ax, cs
    mov cl, 12
    shr ax, cl
    mov [cs_base_hi], al

    ; GDT entry 0: null (already zero)

    ; We need entries for every selector value the CPU might encounter.
    ; CS is our real-mode segment value (e.g. 0x1000).
    ; We'll create the GDT large enough and put a valid descriptor at
    ; the index matching our CS value.
    ;
    ; Selector index = CS & 0xFFF8, byte offset = that value.
    ; For CS=0x1000, index=0x1000, byte offset=0x1000 into the GDT.
    ; That's 4096 bytes — too large for a simple test.
    ;
    ; Simpler approach: far JMP to a code segment with a small selector,
    ; then set up DS/SS with small selectors too.

    ; Entry 1 (selector 0x08): code segment matching our real CS
    mov ax, 0xFFFF
    mov [gdt + 8 + 0], ax
    mov ax, [cs_base_lo]
    mov [gdt + 8 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 8 + 4], al
    mov byte [gdt + 8 + 5], 0x9A  ; present, DPL0, code, exec/read

    ; Entry 2 (selector 0x10): data segment matching our real DS
    mov ax, 0xFFFF
    mov [gdt + 16 + 0], ax
    mov ax, [cs_base_lo]
    mov [gdt + 16 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 16 + 4], al
    mov byte [gdt + 16 + 5], 0x92  ; present, DPL0, data, read/write

    ; Load GDTR
    mov word [gdtr_val], 0x0017    ; limit = 3*8 - 1 = 23
    mov ax, [cs_base_lo]
    add ax, gdt
    mov [gdtr_val + 2], ax
    mov al, [cs_base_hi]
    adc al, 0
    mov [gdtr_val + 4], al
    lgdt [gdtr_val]

    ; Far JMP to load CS with selector 0x08
    jmp 0x0008:.cs_loaded
.cs_loaded:
    ; Load DS and SS with selector 0x10
    mov ax, 0x0010
    mov ds, ax
    mov ss, ax
    ret

;=============================================================================
; Test: SMSW initial value should have PE=0
;=============================================================================
test_smsw_initial:
    mov si, test_smsw_initial_name
    call print_test_name

    smsw ax
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
;=============================================================================
test_smsw_to_memory:
    mov si, test_smsw_to_memory_name
    call print_test_name

    mov word [msw_buffer], 0xFFFF
    smsw [msw_buffer]
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
; Test: LMSW can set MP, EM, TS bits (bits 1-3) without setting PE
;=============================================================================
test_lmsw_set_bits:
    mov si, test_lmsw_set_bits_name
    call print_test_name

    mov ax, 0x000E
    lmsw ax
    smsw ax
    and ax, 0x000E
    cmp ax, 0x000E
    jne .fail

    mov ax, 0x0000
    lmsw ax

    call print_pass
    ret

.fail:
    mov si, msg_lmsw_set_bits_fail
    call print_fail
    mov ax, 0x0000
    lmsw ax
    ret

;=============================================================================
; Helpers
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

msg_summary: db '--- Summary ---', 13, 10, 0
msg_passed:  db ' passed, ', 0
msg_failed:  db ' failed', 0

section .bss
pass_count:  resw 1
fail_count:  resw 1
msw_buffer:  resw 1
cs_base_lo:  resw 1
cs_base_hi:  resb 1
gdt:         resb 24      ; 3 entries * 8 bytes
gdtr_val:    resb 6
