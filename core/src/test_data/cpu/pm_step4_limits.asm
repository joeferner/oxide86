[CPU 286]
[ORG 0x100]

; Protected Mode Step 4: Segment Limit Checking
;
; Tests that in protected mode, memory accesses beyond a segment's limit
; are rejected (#GP), and accesses within the limit succeed.
;
; GDT layout:
;   0x00: null descriptor
;   0x08: code segment — base=CS<<4, limit=0xFFFF, exec/read (DPL 0)
;   0x10: data segment — base=CS<<4, limit=0xFFFF, read/write (DPL 0)
;   0x18: small data segment — base=0x50000, limit=0x000F, read/write
;         (only 16 bytes accessible: offsets 0x0000–0x000F)
;   0x20: verify data segment — base=0x50000, limit=0xFFFF, read/write
;         (same base as small segment but much larger limit, for verification)
;
; Tests:
;   1. Write/read within limit of small segment (offset 0x0000) — should succeed
;   2. Write/read at limit boundary (offset 0x000F) — should succeed
;   3. Write beyond limit (offset 0x0010) — should trigger #GP(0)
;   4. Word write straddling limit (offset 0x000F, word) — should trigger #GP(0)
;   5. Verify data written through small segment is visible via verify segment
;      at the same offset (both have base=0x50000)

section .text
start:
    mov si, msg_banner
    call print_string

    mov word [pass_count], 0
    mov word [fail_count], 0

    call build_gdt
    lgdt [gdtr_value]

    ; Write known values to physical 0x50000 area using real-mode access
    push ds
    mov ax, 0x5000
    mov ds, ax
    ; Clear first 32 bytes
    xor di, di
    mov cx, 32
.clear:
    mov byte [di], 0x00
    inc di
    loop .clear
    pop ds

    ; === Enter protected mode ===
    cli
    mov ax, 0x0001
    lmsw ax
    jmp 0x0008:pm_entry

pm_entry:
    ; Set up data segments
    mov ax, 0x0010
    mov ds, ax
    mov ss, ax
    mov ax, 0x0020
    mov es, ax          ; ES = verify segment (base=0x50000, limit=0xFFFF)

    ; --- Test 1: Write/read within limit of small segment ---
    mov ax, 0x0018
    mov ds, ax          ; DS = small segment (base=0x50000, limit=0x000F)
    mov byte [0x0000], 0xAA  ; offset 0 — within limit
    mov al, [0x0000]
    cmp al, 0xAA
    jne .test1_fail
    inc word [cs:pass_count]
    jmp .test2
.test1_fail:
    inc word [cs:fail_count]

.test2:
    ; --- Test 2: Write/read at limit boundary (offset 0x000F) ---
    mov byte [0x000F], 0xBB  ; offset 15 — exactly at limit for byte access
    mov al, [0x000F]
    cmp al, 0xBB
    jne .test2_fail
    inc word [cs:pass_count]
    jmp .test3
.test2_fail:
    inc word [cs:fail_count]

.test3:
    ; --- Test 3: Write beyond limit (offset 0x0010) should #GP ---
    ; First write a known value via verify segment (same base, larger limit)
    mov byte [es:0x0010], 0x77
    ; Now try to write through small segment at offset 0x10 (beyond limit)
    mov byte [0x0010], 0xFF    ; should be blocked by limit check
    ; Verify via verify segment that the byte is still 0x77
    mov al, [es:0x0010]
    cmp al, 0x77
    jne .test3_fail
    inc word [cs:pass_count]
    jmp .test4
.test3_fail:
    inc word [cs:fail_count]

.test4:
    ; --- Test 4: Word write straddling limit (offset 0x000F) should #GP ---
    ; A word write at offset 0x000F would access bytes 0x000F and 0x0010.
    ; Byte 0x0010 is beyond the limit, so this should be blocked.
    mov byte [es:0x000F], 0xBB  ; restore byte at limit via verify seg
    mov byte [es:0x0010], 0x77  ; known value just past limit
    mov word [0x000F], 0xCCDD   ; should be blocked (straddles limit)
    ; Check that both bytes are unchanged
    mov al, [es:0x000F]
    cmp al, 0xBB
    jne .test4_fail
    mov al, [es:0x0010]
    cmp al, 0x77
    jne .test4_fail
    inc word [cs:pass_count]
    jmp .test5
.test4_fail:
    inc word [cs:fail_count]

.test5:
    ; --- Test 5: Verify cross-segment physical address mapping ---
    ; Write 0x42 through small segment at offset 5
    mov byte [0x0005], 0x42
    ; Read through verify segment at same offset (same base)
    mov al, [es:0x0005]
    cmp al, 0x42
    jne .test5_fail
    inc word [cs:pass_count]
    jmp .done
.test5_fail:
    inc word [cs:fail_count]

.done:
    ; Restore DS/SS for exit
    mov ax, 0x0010
    mov ds, ax
    mov ss, ax

    ; Print summary and exit
    call print_summary

    mov ah, 4Ch
    mov al, [fail_count]
    int 21h

;=============================================================================
; Build GDT
;=============================================================================
build_gdt:
    mov ax, cs
    mov cl, 4
    shl ax, cl
    mov [cs_base_lo], ax
    mov ax, cs
    mov cl, 12
    shr ax, cl
    mov [cs_base_hi], al

    ; Entry 1 (0x08): code segment
    mov word [gdt + 8 + 0], 0xFFFF
    mov ax, [cs_base_lo]
    mov [gdt + 8 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 8 + 4], al
    mov byte [gdt + 8 + 5], 0x9A

    ; Entry 2 (0x10): data segment (same base as code)
    mov word [gdt + 16 + 0], 0xFFFF
    mov ax, [cs_base_lo]
    mov [gdt + 16 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 16 + 4], al
    mov byte [gdt + 16 + 5], 0x92

    ; Entry 3 (0x18): small data segment (base=0x50000, limit=0x000F)
    mov word [gdt + 24 + 0], 0x000F  ; limit = 15 bytes
    mov word [gdt + 24 + 2], 0x0000  ; base low = 0x0000
    mov byte [gdt + 24 + 4], 0x05    ; base high = 0x05 (phys 0x50000)
    mov byte [gdt + 24 + 5], 0x92    ; present, DPL0, data, read/write

    ; Entry 4 (0x20): verify data segment (base=0x50000, limit=0xFFFF)
    mov word [gdt + 32 + 0], 0xFFFF
    mov word [gdt + 32 + 2], 0x0000  ; base low = 0x0000
    mov byte [gdt + 32 + 4], 0x05    ; base high = 0x05 (phys 0x50000)
    mov byte [gdt + 32 + 5], 0x92

    ; GDTR: limit = 5*8-1 = 39 = 0x27
    mov word [gdtr_value], 0x0027
    mov ax, [cs_base_lo]
    add ax, gdt
    mov [gdtr_value + 2], ax
    mov al, [cs_base_hi]
    adc al, 0
    mov [gdtr_value + 4], al

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

msg_banner:  db '=== 286 PM Step 4: Limit Checking ===', 13, 10, 0
msg_pass:    db 'PASS', 13, 10, 0
msg_fail:    db 'FAIL - ', 0
msg_summary: db '--- Summary ---', 13, 10, 0
msg_passed:  db ' passed, ', 0
msg_failed:  db ' failed', 0

section .bss
pass_count:  resw 1
fail_count:  resw 1
cs_base_lo:  resw 1
cs_base_hi:  resb 1
gdt:         resb 40     ; 5 entries * 8 bytes
gdtr_value:  resb 6
