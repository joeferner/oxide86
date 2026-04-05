[CPU 286]
[ORG 0x100]

; Protected Mode Step 3: Descriptor Table Structures and Segment Loading
;
; Tests that in protected mode, segment register loads use GDT selectors
; to look up descriptors, and memory accesses use the descriptor's base
; address instead of (segment << 4).
;
; GDT layout (set up in real mode before switching):
;   Selector 0x00: null descriptor (required)
;   Selector 0x08: code segment — base=real CS<<4, limit=0xFFFF, exec/read
;   Selector 0x10: data segment — base=real DS<<4, limit=0xFFFF, read/write
;   Selector 0x18: data segment — base=0x00000, limit=0xFFFF, read/write
;                  (flat segment starting at physical 0)
;   Selector 0x20: data segment — base=0x50000, limit=0xFFFF, read/write
;                  (offset segment: DS:0 maps to physical 0x50000)
;
; Strategy:
;   1. In real mode, write a known byte to physical address 0x50000
;   2. Enter protected mode
;   3. Load DS with selector 0x20 (base=0x50000)
;   4. Read DS:[0x0000] — should read from physical 0x50000
;   5. Load ES with selector 0x18 (base=0x00000)
;   6. Write a byte through ES:[0x50010] — should hit physical 0x50010
;   7. Load DS with selector 0x18 (base=0x00000)
;   8. Read DS:[0x50010] — should match what we wrote through ES
;   9. Also test that loading a null selector (0x0000) into DS works
;      (it's allowed, but accessing memory through it should #GP — tested
;       in a later step once exceptions are wired up)

section .text
start:
    mov si, msg_banner
    call print_string

    mov word [pass_count], 0
    mov word [fail_count], 0

    ; ---- Prepare: write known values to physical memory while in real mode ----
    ; Write 0xAA to physical 0x50000 using segment 0x5000:0x0000
    push ds
    mov ax, 0x5000
    mov ds, ax
    mov byte [0x0000], 0xAA
    ; Write 0x55 to physical 0x50010 using segment 0x5000:0x0010
    mov byte [0x0010], 0x55
    pop ds

    ; ---- Build GDT in memory ----
    call build_gdt

    ; ---- Load GDTR ----
    lgdt [gdtr_value]

    ; ---- Tests that run in protected mode ----
    ; We need to print test names BEFORE entering PM since INT 21h won't
    ; work in PM. Instead, we'll do all work in PM, store results, return
    ; to real mode style (just clear PE for testing purposes since the
    ; emulator doesn't enforce full PM yet), then print results.
    ;
    ; Actually, for Step 3 testing: the emulator needs to handle segment
    ; loads in PM. We enter PM, do segment loads and memory accesses,
    ; store results to known locations, then we rely on the test framework.
    ;
    ; Since this is TDD and we can't rely on INT 21h in PM, we'll test
    ; by entering PM, doing our work, and halting. The test checks exit code.
    ;
    ; Simpler approach: enter PM, do segment loads and reads, store results
    ; in a results buffer (using a flat segment), exit PM, then check results
    ; and print in real mode.

    ; --- Enter protected mode ---
    ; First, disable interrupts (no IDT set up)
    cli

    ; Set PE
    mov ax, 0x0001
    lmsw ax

    ; Far jump to load CS with selector 0x08 (code segment)
    jmp 0x0008:pm_entry

pm_entry:
    ; Now in protected mode with CS=0x08

    ; --- Test 1: Load DS with selector 0x20 (base=0x50000), read DS:[0] ---
    mov ax, 0x0020
    mov ds, ax
    mov al, [0x0000]       ; Should read from physical 0x50000
    mov [cs:result_1], al  ; Store result using CS-relative (code seg)

    ; --- Test 2: Load ES with selector 0x18 (base=0x00000), write ES:[0x50010] ---
    mov ax, 0x0018
    mov es, ax
    mov byte [es:0x50010], 0xBB  ; Should write to physical 0x50010

    ; --- Test 3: Load DS with selector 0x18 (base=0x00000), read DS:[0x50010] ---
    mov ax, 0x0018
    mov ds, ax
    mov al, [0x50010]      ; Should read from physical 0x50010 (the 0xBB we just wrote)
    mov [cs:result_3], al

    ; --- Test 4: Load DS with selector 0x10 (original data segment base) ---
    ; Read from the data area to verify it works
    mov ax, 0x0010
    mov ds, ax
    mov al, [result_1]     ; Should be accessible since base matches real DS<<4
    mov [cs:result_4], al  ; Copy it (should be same as result_1)

    ; --- Test 5: Load null selector into ES (allowed, no fault) ---
    xor ax, ax
    mov es, ax
    mov byte [cs:result_5], 0x01  ; Mark that we got here without faulting

    ; --- Exit protected mode ---
    ; Clear PE (the emulator allows this for testing even though real 286 doesn't)
    ; NOTE: In Step 1, LMSW can't clear PE on 286, so we need to just stay in PM
    ; and set up SS/DS to workable selectors before exiting via HLT or
    ; we can use a different approach. For now, let's keep it simple:
    ; restore DS to selector 0x10 (our data segment), check results, and exit
    ; via INT 21h — but INT 21h requires real mode...
    ;
    ; Simplest approach: we've stored all results. Now HLT.
    ; The Rust test will check exit differently.
    ;
    ; Alternative: since we're testing the emulator incrementally, and at this
    ; step the emulator doesn't fully enforce PM (no IDT, no #GP), we can
    ; attempt INT 21h with DS pointing to our data segment.

    ; Restore DS to selector 0x10 (data segment matching our original DS base)
    mov ax, 0x0010
    mov ds, ax

    ; Restore SS to selector 0x10 as well (stack is in our data area)
    mov ax, 0x0010
    mov ss, ax

    ; Now check results and set exit code
    mov ah, 0

    ; Check result_1: should be 0xAA
    cmp byte [result_1], 0xAA
    je .r1_ok
    inc ah
.r1_ok:

    ; Check result_3: should be 0xBB
    cmp byte [result_3], 0xBB
    je .r3_ok
    inc ah
.r3_ok:

    ; Check result_4: should be same as result_1 (0xAA)
    cmp byte [result_4], 0xAA
    je .r4_ok
    inc ah
.r4_ok:

    ; Check result_5: should be 0x01 (null selector load succeeded)
    cmp byte [result_5], 0x01
    je .r5_ok
    inc ah
.r5_ok:

    ; AH now holds the failure count. Exit with it.
    mov al, ah
    mov ah, 4Ch
    int 21h

;=============================================================================
; Build the GDT in memory
; Called in real mode before entering protected mode
;=============================================================================
build_gdt:
    ; We need the physical base of our code/data segment to build descriptors.
    ; In real mode, CS and DS both point to our segment. Physical base = seg << 4.
    ; For a .com file loaded at TEST_SEGMENT:0x0100, the segment is TEST_SEGMENT.
    ;
    ; We'll compute it: base = CS << 4
    mov ax, cs
    ; Multiply by 16: shift left 4
    mov cl, 4
    shl ax, cl           ; AX = low 16 bits of (CS << 4)
    mov [cs_base_lo], ax
    ; High byte: CS >> 12
    mov ax, cs
    mov cl, 12
    shr ax, cl
    mov [cs_base_hi], al

    ; Entry 0: Null descriptor (8 bytes of zero) — already zero in BSS

    ; Entry 1 (selector 0x08): Code segment
    ; base = CS<<4, limit = 0xFFFF, access = 0x9A (present, DPL0, exec/read)
    mov ax, 0xFFFF
    mov [gdt + 8 + 0], ax        ; limit low
    mov ax, [cs_base_lo]
    mov [gdt + 8 + 2], ax        ; base low
    mov al, [cs_base_hi]
    mov [gdt + 8 + 4], al        ; base high (byte 2 of 3)
    mov byte [gdt + 8 + 5], 0x9A ; access: P=1, DPL=0, S=1, type=exec/read

    ; Entry 2 (selector 0x10): Data segment (same base as CS)
    ; base = CS<<4 (same segment for .com), limit = 0xFFFF, access = 0x92
    mov ax, 0xFFFF
    mov [gdt + 16 + 0], ax
    mov ax, [cs_base_lo]
    mov [gdt + 16 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 16 + 4], al
    mov byte [gdt + 16 + 5], 0x92 ; access: P=1, DPL=0, S=1, type=data/read-write

    ; Entry 3 (selector 0x18): Flat data segment (base=0x00000)
    ; base = 0, limit = 0xFFFF, access = 0x92
    mov word [gdt + 24 + 0], 0xFFFF  ; limit
    mov word [gdt + 24 + 2], 0x0000  ; base low = 0
    mov byte [gdt + 24 + 4], 0x00    ; base high = 0
    mov byte [gdt + 24 + 5], 0x92    ; access

    ; Entry 4 (selector 0x20): Offset data segment (base=0x50000)
    ; base = 0x50000, limit = 0xFFFF, access = 0x92
    mov word [gdt + 32 + 0], 0xFFFF  ; limit
    mov word [gdt + 32 + 2], 0x0000  ; base low = 0x0000 (low 16 of 0x50000)
    mov byte [gdt + 32 + 4], 0x05    ; base high = 0x05 (byte 2: bits 16-23 of 0x50000)
    mov byte [gdt + 32 + 5], 0x92    ; access

    ; Set up GDTR value
    ; limit = 5 entries * 8 - 1 = 39 = 0x27
    mov word [gdtr_value], 0x0027
    ; base = physical address of gdt = CS<<4 + offset of gdt
    mov ax, [cs_base_lo]
    add ax, gdt
    mov [gdtr_value + 2], ax      ; base low 16
    ; Carry into high byte
    mov al, [cs_base_hi]
    adc al, 0
    mov [gdtr_value + 4], al      ; base bits 16-23

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

msg_banner: db '=== 286 PM Step 3: Segment Loading ===', 13, 10, 0
msg_pass:   db 'PASS', 13, 10, 0
msg_fail:   db 'FAIL - ', 0
msg_summary: db '--- Summary ---', 13, 10, 0
msg_passed:  db ' passed, ', 0
msg_failed:  db ' failed', 0

section .bss
pass_count:  resw 1
fail_count:  resw 1

; GDT: 5 entries * 8 bytes = 40 bytes
gdt:         resb 40

; GDTR value: 6 bytes (limit16 + base24 + reserved8)
gdtr_value:  resb 6

; Temporaries for base calculation
cs_base_lo:  resw 1
cs_base_hi:  resb 1

; Results stored by PM code, checked after returning
result_1:    resb 1     ; Test 1: DS:[0] with base=0x50000, expect 0xAA
result_3:    resb 1     ; Test 3: DS:[0x50010] with base=0, expect 0xBB
result_4:    resb 1     ; Test 4: DS with original base, read result_1, expect 0xAA
result_5:    resb 1     ; Test 5: null selector load succeeded, expect 0x01
