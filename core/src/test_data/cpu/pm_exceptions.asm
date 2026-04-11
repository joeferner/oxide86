[CPU 286]
[ORG 0x100]

; Protected Mode Step 6: Exception Handling (#GP, #NP)
;
; Tests that CPU exceptions in protected mode dispatch through the IDT
; and push an error code onto the stack.
;
; GDT layout:
;   0x00: null descriptor
;   0x08: code segment — base=CS<<4, limit=0xFFFF, exec/read, DPL 0
;   0x10: data segment — base=CS<<4, limit=0xFFFF, read/write, DPL 0
;   0x18: small data segment — base=CS<<4, limit=0x000F, read/write, DPL 0
;   0x20: not-present data segment — base=CS<<4, limit=0xFFFF, P=0
;
; IDT layout:
;   Entry 0x0B (INT 0Bh): #NP handler — interrupt gate → 0x0008:handler_np
;   Entry 0x0D (INT 0Dh): #GP handler — interrupt gate → 0x0008:handler_gp
;
; Tests:
;   1. #GP fires on segment limit violation (write beyond limit)
;      — handler runs, error code = 0 (no specific selector)
;   2. #GP handler receives correct error code on stack
;   3. #NP fires when loading a not-present segment selector
;      — handler runs, error code = selector that caused the fault
;   4. #GP fires on loading an out-of-bounds selector
;   5. Execution continues correctly after exception handler returns

section .text
start:
    mov word [pass_count], 0
    mov word [fail_count], 0

    call build_gdt
    call build_idt
    lgdt [gdtr_value]
    lidt [idtr_value]

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

    ; --- Test 1: #GP on segment limit violation ---
    ; Load DS with small segment (limit=0x000F), write at offset 0x20
    mov word [gp_fired], 0
    mov word [gp_error_code], 0xFFFF
    mov ax, 0x0018
    mov ds, ax
    mov byte [0x0020], 0xFF     ; offset 0x20 > limit 0x0F → #GP(0)

    ; Restore DS for checking results
    mov ax, 0x0010
    mov ds, ax

    cmp word [gp_fired], 1
    jne .test1_fail
    inc word [pass_count]
    jmp .test2
.test1_fail:
    inc word [fail_count]

.test2:
    ; --- Test 2: #GP error code is 0 for limit violation ---
    cmp word [gp_error_code], 0x0000
    je .test2_pass
    inc word [fail_count]
    jmp .test3
.test2_pass:
    inc word [pass_count]

.test3:
    ; --- Test 3: #NP fires when loading a not-present segment ---
    mov word [np_fired], 0
    mov word [np_error_code], 0xFFFF
    mov ax, 0x0020              ; selector for not-present segment
    mov es, ax                  ; should trigger #NP(0x0020)

    cmp word [np_fired], 1
    jne .test3_fail
    inc word [pass_count]
    jmp .test4
.test3_fail:
    inc word [fail_count]

.test4:
    ; --- Test 4: #NP error code is the faulting selector ---
    cmp word [np_error_code], 0x0020
    je .test4_pass
    inc word [fail_count]
    jmp .test5
.test4_pass:
    inc word [pass_count]

.test5:
    ; --- Test 5: #GP fires on loading an out-of-bounds selector ---
    mov word [gp_fired], 0
    mov word [gp_error_code], 0xFFFF
    mov ax, 0x0F00              ; selector way beyond GDT limit
    mov es, ax                  ; should trigger #GP(0x0F00)

    cmp word [gp_fired], 1
    jne .test5_fail
    inc word [pass_count]
    jmp .done
.test5_fail:
    inc word [fail_count]

.done:
    ; Exit: load empty IDTR so INT 21h falls back to real-mode IVT
    lidt [empty_idtr]
    mov ah, 4Ch
    mov al, [fail_count]
    int 21h

;=============================================================================
; #GP handler (INT 0Dh)
; Stack on entry: [SP+0]=error_code, [SP+2]=IP, [SP+4]=CS, [SP+6]=FLAGS
;=============================================================================
handler_gp:
    push ax
    push bp
    mov bp, sp
    ; error code is at [bp+4] (above saved ax and bp)
    mov ax, [bp + 4]
    mov [cs:gp_error_code], ax
    mov word [cs:gp_fired], 1
    pop bp
    pop ax
    add sp, 2                   ; pop error code
    iret

;=============================================================================
; #NP handler (INT 0Bh)
; Stack on entry: [SP+0]=error_code, [SP+2]=IP, [SP+4]=CS, [SP+6]=FLAGS
;=============================================================================
handler_np:
    push ax
    push bp
    mov bp, sp
    mov ax, [bp + 4]
    mov [cs:np_error_code], ax
    mov word [cs:np_fired], 1
    pop bp
    pop ax
    add sp, 2                   ; pop error code
    iret

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
    mov byte [gdt + 8 + 5], 0x9A  ; P=1, DPL=0, code, exec/read

    ; Entry 2 (0x10): data segment
    mov word [gdt + 16 + 0], 0xFFFF
    mov ax, [cs_base_lo]
    mov [gdt + 16 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 16 + 4], al
    mov byte [gdt + 16 + 5], 0x92  ; P=1, DPL=0, data, read/write

    ; Entry 3 (0x18): small data segment (limit=0x000F)
    mov word [gdt + 24 + 0], 0x000F
    mov ax, [cs_base_lo]
    mov [gdt + 24 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 24 + 4], al
    mov byte [gdt + 24 + 5], 0x92  ; P=1, DPL=0, data, read/write

    ; Entry 4 (0x20): not-present data segment
    mov word [gdt + 32 + 0], 0xFFFF
    mov ax, [cs_base_lo]
    mov [gdt + 32 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 32 + 4], al
    mov byte [gdt + 32 + 5], 0x12  ; P=0, DPL=0, data, read/write

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
; Build IDT
; Need entries for INT 0Bh (#NP) and INT 0Dh (#GP).
; INT 0Bh at offset 0x0B * 8 = 0x58
; INT 0Dh at offset 0x0D * 8 = 0x68
; Minimum IDT size: 0x6F + 1 = 0x70 bytes (limit = 0x6F)
;=============================================================================
build_idt:
    ; INT 0Bh (#NP): interrupt gate
    mov ax, handler_np
    mov [idt + 0x58 + 0], ax
    mov word [idt + 0x58 + 2], 0x0008  ; selector = code segment
    mov byte [idt + 0x58 + 4], 0x00
    mov byte [idt + 0x58 + 5], 0x86    ; P=1, DPL=0, 286 interrupt gate
    mov word [idt + 0x58 + 6], 0x0000

    ; INT 0Dh (#GP): interrupt gate
    mov ax, handler_gp
    mov [idt + 0x68 + 0], ax
    mov word [idt + 0x68 + 2], 0x0008
    mov byte [idt + 0x68 + 4], 0x00
    mov byte [idt + 0x68 + 5], 0x86    ; P=1, DPL=0, 286 interrupt gate
    mov word [idt + 0x68 + 6], 0x0000

    ; IDTR: limit = 0x6F (covers through INT 0Dh)
    mov word [idtr_value], 0x006F
    mov ax, [cs_base_lo]
    add ax, idt
    mov [idtr_value + 2], ax
    mov al, [cs_base_hi]
    adc al, 0
    mov [idtr_value + 4], al

    ; Empty IDTR for exit
    mov word [empty_idtr], 0x0000
    mov word [empty_idtr + 2], 0x0000
    mov word [empty_idtr + 4], 0x0000
    ret

;=============================================================================
; Data
;=============================================================================
section .data

empty_idtr:
    dw 0x0000
    db 0x00, 0x00, 0x00, 0x00

section .bss
pass_count:     resw 1
fail_count:     resw 1
cs_base_lo:     resw 1
cs_base_hi:     resb 1
gdt:            resb 40        ; 5 entries * 8
gdtr_value:     resb 6
idt:            resb 0x70      ; covers INT 0x00–0x0D
idtr_value:     resb 6
gp_fired:       resw 1
gp_error_code:  resw 1
np_fired:       resw 1
np_error_code:  resw 1
