[CPU 286]
[ORG 0x100]

; Protected Mode Step 7: LLDT / SLDT / LTR / STR
;
; Tests the LDT and Task Register instructions.
;
; GDT layout:
;   0x00: null descriptor
;   0x08: code segment — base=CS<<4, limit=0xFFFF, exec/read, DPL 0
;   0x10: data segment — base=CS<<4, limit=0xFFFF, read/write, DPL 0
;   0x18: LDT descriptor — base=physical addr of ldt_table, limit=0x0F (2 entries)
;   0x20: TSS descriptor — base=physical addr of tss_data, limit=43, type=available TSS
;
; LDT layout (at ldt_table):
;   0x00: null descriptor
;   0x08: data segment — base=0x60000, limit=0xFFFF, read/write, DPL 0
;
; Tests:
;   1. SLDT returns 0 before any LLDT
;   2. LLDT loads the LDT register, SLDT reads it back
;   3. Load a segment from the LDT (selector with TI=1), access memory through it
;   4. STR returns 0 before any LTR
;   5. LTR loads the Task Register, STR reads it back

section .text
start:
    mov word [pass_count], 0
    mov word [fail_count], 0

    call build_gdt
    call build_ldt
    lgdt [gdtr_value]

    ; === Enter protected mode ===
    cli
    mov ax, 0x0001
    lmsw ax
    jmp 0x0008:pm_entry

pm_entry:
    mov ax, 0x0010
    mov ds, ax
    mov ss, ax
    mov es, ax

    ; --- Test 1: SLDT returns 0 before LLDT ---
    mov ax, 0xFFFF          ; poison
    sldt ax
    cmp ax, 0x0000
    jne .test1_fail
    inc word [pass_count]
    jmp .test2
.test1_fail:
    inc word [fail_count]

.test2:
    ; --- Test 2: LLDT then SLDT ---
    mov ax, 0x0018          ; selector for LDT descriptor in GDT
    lldt ax
    mov ax, 0xFFFF          ; poison
    sldt ax
    cmp ax, 0x0018
    jne .test2_fail
    inc word [pass_count]
    jmp .test3
.test2_fail:
    inc word [fail_count]

.test3:
    ; --- Test 3: Load segment from LDT, access memory ---
    ; Write a known byte to physical 0x60000 using GDT data segment
    ; First, set up ES with a GDT segment that covers 0x60000
    ; Our data segment base is CS<<4 which doesn't reach 0x60000.
    ; Instead, write via real-mode-compatible addressing:
    ; We'll write directly to the physical address by using a
    ; special GDT entry — but we don't have one for 0x60000 in GDT.
    ;
    ; Simpler: write the known value BEFORE entering PM.
    ; Actually we're already in PM. Let's use the LDT segment itself.
    ;
    ; Load DS with LDT selector 0x0C (TI=1, index=1 → LDT entry at offset 8)
    ; Selector = (index << 3) | TI | RPL = (1 << 3) | 0x04 | 0x00 = 0x000C
    mov ax, 0x000C
    mov ds, ax              ; DS = LDT entry 1 (base=0x60000, limit=0xFFFF)
    ; Write through this segment
    mov byte [0x0000], 0xCC ; write to physical 0x60000
    mov al, [0x0000]        ; read it back
    cmp al, 0xCC
    jne .test3_fail

    ; Also write at a different offset and verify
    mov byte [0x0010], 0xDD
    mov al, [0x0010]
    cmp al, 0xDD
    jne .test3_fail

    ; Restore DS
    mov ax, 0x0010
    mov ds, ax
    inc word [pass_count]
    jmp .test4
.test3_fail:
    mov ax, 0x0010
    mov ds, ax
    inc word [fail_count]

.test4:
    ; --- Test 4: STR returns 0 before LTR ---
    mov ax, 0xFFFF
    str ax
    cmp ax, 0x0000
    jne .test4_fail
    inc word [pass_count]
    jmp .test5
.test4_fail:
    inc word [fail_count]

.test5:
    ; --- Test 5: LTR then STR ---
    mov ax, 0x0020          ; selector for TSS descriptor in GDT
    ltr ax
    mov ax, 0xFFFF
    str ax
    cmp ax, 0x0020
    jne .test5_fail
    inc word [pass_count]
    jmp .done
.test5_fail:
    inc word [fail_count]

.done:
    ; Exit
    lidt [empty_idtr]
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

    ; Entry 2 (0x10): data segment
    mov word [gdt + 16 + 0], 0xFFFF
    mov ax, [cs_base_lo]
    mov [gdt + 16 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 16 + 4], al
    mov byte [gdt + 16 + 5], 0x92

    ; Entry 3 (0x18): LDT descriptor
    ; type = 0x02 (LDT), S=0, P=1, DPL=0 → access = 0x82
    mov word [gdt + 24 + 0], 0x000F     ; limit = 15 (2 entries * 8 - 1)
    ; base = physical address of ldt_table
    mov ax, [cs_base_lo]
    add ax, ldt_table
    mov [gdt + 24 + 2], ax              ; base low
    mov al, [cs_base_hi]
    adc al, 0
    mov [gdt + 24 + 4], al              ; base high
    mov byte [gdt + 24 + 5], 0x82       ; P=1, DPL=0, S=0, type=LDT

    ; Entry 4 (0x20): TSS descriptor (available 286 TSS)
    ; type = 0x01 (available 286 TSS), S=0, P=1, DPL=0 → access = 0x81
    mov word [gdt + 32 + 0], 43         ; limit = 43 (minimum 286 TSS size)
    mov ax, [cs_base_lo]
    add ax, tss_data
    mov [gdt + 32 + 2], ax              ; base low
    mov al, [cs_base_hi]
    adc al, 0
    mov [gdt + 32 + 4], al              ; base high
    mov byte [gdt + 32 + 5], 0x81       ; P=1, DPL=0, S=0, type=available 286 TSS

    ; GDTR: limit = 5*8-1 = 39
    mov word [gdtr_value], 0x0027
    mov ax, [cs_base_lo]
    add ax, gdt
    mov [gdtr_value + 2], ax
    mov al, [cs_base_hi]
    adc al, 0
    mov [gdtr_value + 4], al
    ret

;=============================================================================
; Build LDT
;=============================================================================
build_ldt:
    ; Entry 0: null (already zero in BSS)

    ; Entry 1 (LDT-local selector 0x0C): data segment at base=0x60000
    mov word [ldt_table + 8 + 0], 0xFFFF  ; limit
    mov word [ldt_table + 8 + 2], 0x0000  ; base low = 0x0000
    mov byte [ldt_table + 8 + 4], 0x06    ; base high = 0x06 (phys 0x60000)
    mov byte [ldt_table + 8 + 5], 0x92    ; P=1, DPL=0, data, read/write
    ret

;=============================================================================
; Data
;=============================================================================
section .data

empty_idtr:
    dw 0x0000
    db 0x00, 0x00, 0x00, 0x00

section .bss
pass_count:  resw 1
fail_count:  resw 1
cs_base_lo:  resw 1
cs_base_hi:  resb 1
gdt:         resb 40        ; 5 entries * 8
gdtr_value:  resb 6
ldt_table:   resb 16        ; 2 entries * 8
tss_data:    resb 44        ; minimum 286 TSS
