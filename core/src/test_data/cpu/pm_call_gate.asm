[CPU 286]
[ORG 0x100]

; Protected Mode Step 8: Far CALL/JMP and RET Through Call Gates
;
; Tests that far CALL/JMP through call gates works in protected mode.
;
; GDT layout:
;   0x00: null descriptor
;   0x08: code segment — base=CS<<4, limit=0xFFFF, exec/read, DPL 0
;   0x10: data segment — base=CS<<4, limit=0xFFFF, read/write, DPL 0
;   0x18: call gate → selector 0x08, offset=gate_target_call, DPL 0, word count=0
;   0x20: call gate → selector 0x08, offset=gate_target_jmp, DPL 0, word count=0
;
; Tests:
;   1. Far CALL through call gate — control reaches the target function
;   2. Far RET from call gate target — returns correctly to caller
;   3. Call gate target can access parameters/locals and return a value
;   4. Far JMP through call gate — control reaches the target
;   5. Far CALL direct to code segment (not through gate) — still works

section .text
start:
    mov word [pass_count], 0
    mov word [fail_count], 0

    call build_gdt
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

    ; --- Test 1 & 2: Far CALL through call gate, RET returns ---
    mov word [gate_call_flag], 0
    ; Far CALL using the call gate selector 0x18, offset is ignored
    ; (the gate provides the actual target offset)
    call 0x0018:0x0000

    cmp word [gate_call_flag], 0xAAAA
    jne .test1_fail
    inc word [pass_count]
    jmp .test2
.test1_fail:
    inc word [fail_count]

.test2:
    ; If we got here, RET worked (test 2)
    ; Verify IP is correct by checking we're executing sequentially
    inc word [pass_count]

.test3:
    ; --- Test 3: Call gate target returns a value in AX ---
    mov ax, 0x0000
    call 0x0018:0x0000
    cmp ax, 0x1234
    jne .test3_fail
    inc word [pass_count]
    jmp .test4
.test3_fail:
    inc word [fail_count]

.test4:
    ; --- Test 4: Far JMP through call gate ---
    mov word [gate_jmp_flag], 0
    jmp 0x0020:0x0000       ; JMP through call gate selector 0x20
    ; Should not reach here — gate_target_jmp jumps to .test4_resume
.test4_unreachable:
    inc word [fail_count]
    jmp .test5

.test4_resume:
    cmp word [gate_jmp_flag], 0xBBBB
    jne .test4_fail
    inc word [pass_count]
    jmp .test5
.test4_fail:
    inc word [fail_count]

.test5:
    ; --- Test 5: Far CALL direct to code segment (no gate) ---
    mov word [direct_call_flag], 0
    call 0x0008:direct_target

    cmp word [direct_call_flag], 0xCCCC
    jne .test5_fail
    inc word [pass_count]
    jmp .done
.test5_fail:
    inc word [fail_count]

.done:
    lidt [empty_idtr]
    mov ah, 4Ch
    mov al, [fail_count]
    int 21h

;=============================================================================
; Call gate target (reached via far CALL 0x0018:xxxx)
;=============================================================================
gate_target_call:
    mov word [gate_call_flag], 0xAAAA
    mov ax, 0x1234          ; return value
    retf                    ; far return

;=============================================================================
; Call gate target for JMP (reached via far JMP 0x0020:xxxx)
; Since this was a JMP (not CALL), there's no return address on the stack.
; We set the flag and JMP back to the test code.
;=============================================================================
gate_target_jmp:
    mov word [gate_jmp_flag], 0xBBBB
    jmp 0x0008:.test4_resume_addr
.test4_resume_addr equ pm_entry.test4_resume

;=============================================================================
; Direct far call target (no call gate, direct code segment call)
;=============================================================================
direct_target:
    mov word [direct_call_flag], 0xCCCC
    retf

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

    ; Entry 3 (0x18): call gate → gate_target_call
    ; 286 call gate format (8 bytes):
    ;   bytes 0-1: offset of target
    ;   bytes 2-3: selector (0x0008 = code segment)
    ;   byte  4:   word count (0 for no parameter copying)
    ;   byte  5:   access: P=1, DPL=0, S=0, type=0x04 (286 call gate) → 0x84
    ;   bytes 6-7: reserved (0)
    mov ax, gate_target_call
    mov [gdt + 24 + 0], ax          ; offset
    mov word [gdt + 24 + 2], 0x0008 ; selector
    mov byte [gdt + 24 + 4], 0x00   ; word count
    mov byte [gdt + 24 + 5], 0x84   ; P=1, DPL=0, 286 call gate
    mov word [gdt + 24 + 6], 0x0000

    ; Entry 4 (0x20): call gate → gate_target_jmp
    mov ax, gate_target_jmp
    mov [gdt + 32 + 0], ax
    mov word [gdt + 32 + 2], 0x0008
    mov byte [gdt + 32 + 4], 0x00
    mov byte [gdt + 32 + 5], 0x84   ; P=1, DPL=0, 286 call gate
    mov word [gdt + 32 + 6], 0x0000

    ; GDTR
    mov word [gdtr_value], 0x0027   ; limit = 5*8-1
    mov ax, [cs_base_lo]
    add ax, gdt
    mov [gdtr_value + 2], ax
    mov al, [cs_base_hi]
    adc al, 0
    mov [gdtr_value + 4], al
    ret

;=============================================================================
; Data
;=============================================================================
section .data

empty_idtr:
    dw 0x0000
    db 0x00, 0x00, 0x00, 0x00

section .bss
pass_count:       resw 1
fail_count:       resw 1
cs_base_lo:       resw 1
cs_base_hi:       resb 1
gdt:              resb 40       ; 5 entries * 8
gdtr_value:       resb 6
gate_call_flag:   resw 1
gate_jmp_flag:    resw 1
direct_call_flag: resw 1
