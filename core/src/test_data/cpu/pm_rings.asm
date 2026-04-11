[CPU 286]
[ORG 0x100]

; Protected Mode Step 9: Privilege Level Transitions (Ring 0 ↔ Ring 3)
;
; Tests CPL/DPL checking and ring transitions via IRET and call gates.
;
; GDT layout:
;   0x00: null descriptor
;   0x08: ring 0 code — base=CS<<4, limit=0xFFFF, DPL 0, exec/read
;   0x10: ring 0 data — base=CS<<4, limit=0xFFFF, DPL 0, read/write
;   0x18: ring 3 code — base=CS<<4, limit=0xFFFF, DPL 3, exec/read
;   0x20: ring 3 data — base=CS<<4, limit=0xFFFF, DPL 3, read/write
;   0x28: ring 3 stack — base=CS<<4, limit=0xFFFF, DPL 3, read/write
;   0x30: call gate → ring 0 code (selector 0x08), DPL 3, 0 params
;   0x38: TSS descriptor — for ring 0 SS:SP in the TSS
;
; IDT layout:
;   Entry 0x0D (#GP): interrupt gate → ring 0 handler
;
; Tests:
;   1. IRET from ring 0 to ring 3 — transition to CPL 3 via IRET
;      that pushes ring 3 SS:SP, CS:IP, FLAGS
;   2. In ring 3, verify CPL=3 by reading CS (low 2 bits = RPL = CPL)
;   3. In ring 3, data segment access with DPL 3 works
;   4. Call gate from ring 3 to ring 0 — CPL changes to 0,
;      stack switches to ring 0 SS:SP from TSS
;   5. After returning to ring 0 via call gate, verify CPL=0

section .text
start:
    mov word [pass_count], 0
    mov word [fail_count], 0

    call build_gdt
    call build_idt
    call build_tss
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

    ; Load TSS
    mov ax, 0x0038
    ltr ax

    ; --- Test 1: IRET from ring 0 to ring 3 ---
    ; To transition to ring 3, we do a fake IRET that pops:
    ;   SS:SP (ring 3 stack), FLAGS, CS:IP (ring 3 code)
    ; Push in reverse order: SS, SP, FLAGS, CS, IP
    push word 0x0028 | 3       ; ring 3 SS (selector 0x28, RPL=3 → 0x2B)
    push word 0xFFF0           ; ring 3 SP
    pushf
    pop ax                     ; get FLAGS
    or ax, 0x0200              ; set IF
    push ax                    ; push modified FLAGS
    push word 0x0018 | 3       ; ring 3 CS (selector 0x18, RPL=3 → 0x1B)
    push word ring3_entry      ; ring 3 IP
    iret                       ; transition to ring 3!

    ; Should not reach here
    hlt

;=============================================================================
; Ring 3 code — runs at CPL 3
;=============================================================================
ring3_entry:
    ; --- Test 1 result: if we got here, the transition worked ---
    ; We need DPL 3 data segment to access our variables
    mov ax, 0x0020 | 3         ; ring 3 data selector with RPL=3 → 0x23
    mov ds, ax
    mov es, ax

    ; Verify IRET popped ring 3 SS:SP
    ; SS should be 0x002B (selector 0x28 | RPL 3) and SP should be 0xFFF0
    mov ax, ss
    cmp ax, 0x002B
    jne near .test1_fail
    mov ax, sp
    cmp ax, 0xFFF0
    jne near .test1_fail
    inc word [pass_count]
    jmp near .test2
.test1_fail:
    inc word [fail_count]

.test2:
    ; --- Test 2: Verify CPL=3 by reading CS ---
    mov ax, cs
    and ax, 0x0003              ; RPL bits = CPL
    cmp ax, 3
    jne near .test2_fail
    inc word [pass_count]
    jmp near .test3
.test2_fail:
    inc word [fail_count]

.test3:
    ; --- Test 3: DPL 3 data access works ---
    mov byte [test3_byte], 0xEE
    cmp byte [test3_byte], 0xEE
    jne near .test3_fail
    inc word [pass_count]
    jmp near .test4
.test3_fail:
    inc word [fail_count]

.test4:
    ; --- Test 4: Call gate from ring 3 to ring 0 ---
    ; The call gate at selector 0x30 has DPL=3 so ring 3 can use it.
    ; It targets ring 0 code segment 0x08.
    ; The CPU should switch stacks using the TSS ring 0 SS:SP.
    mov word [gate_r0_flag], 0
    db 0x9A                     ; CALL far opcode
    dw 0x0000                   ; offset (ignored for call gate)
    dw 0x0033                   ; selector 0x30 | RPL 3
    ; If we get here, the call gate returned successfully
    cmp word [gate_r0_flag], 0xD00D
    jne near .test4_fail
    inc word [pass_count]
    jmp near .test5
.test4_fail:
    inc word [fail_count]

.test5:
    ; --- Test 5: After call gate return, verify we're back in ring 3 ---
    mov ax, cs
    and ax, 0x0003
    cmp ax, 3
    jne near .test5_fail
    inc word [pass_count]
    jmp near .done_ring3
.test5_fail:
    inc word [fail_count]

.done_ring3:
    ; Return to ring 0 for exit via call gate
    db 0x9A                     ; CALL far opcode
    dw 0x0000                   ; offset (ignored for call gate)
    dw 0x0033                   ; selector 0x30 | RPL 3
    ; Should not reach here in the normal flow, but just in case:
    jmp near .halt_ring3

.halt_ring3:
    hlt

;=============================================================================
; Ring 0 call gate target
; Reached via call gate from ring 3.
; On entry: stack has been switched to ring 0 stack (from TSS),
; ring 3 SS:SP and return CS:IP are on the ring 0 stack.
;=============================================================================
gate_target_r0:
    ; We're now in ring 0
    ; Load ring 0 data segment
    mov ax, 0x0010
    mov ds, ax

    ; Check if this is the first or second call
    cmp word [gate_r0_flag], 0
    jne .second_call

    ; First call: verify stack switch happened
    ; SS should be ring 0 stack selector (0x0010)
    mov ax, ss
    cmp ax, 0x0010
    jne .stack_switch_fail
    ; SP should be near TSS SP0 value (0xFFC0), minus what was pushed
    ; The CPU pushes: ring3_SS, ring3_SP, ring3_CS, ring3_IP = 4 words = 8 bytes
    ; So SP should be 0xFFC0 - 8 = 0xFFB8
    mov ax, sp
    cmp ax, 0xFFB8
    jne .stack_switch_fail

    mov word [gate_r0_flag], 0xD00D
    retf                        ; far return to ring 3 (inter-privilege return)

.stack_switch_fail:
    ; Stack switch didn't happen — set a different flag value
    mov word [gate_r0_flag], 0xBAD0
    retf

.second_call:
    ; Second call: exit the program
    ; We're in ring 0, load empty IDTR and exit via INT 21h
    lidt [empty_idtr]
    mov ah, 4Ch
    mov al, [fail_count]
    int 21h

;=============================================================================
; #GP handler (ring 0) — for debugging unexpected faults
;=============================================================================
handler_gp:
    ; Just increment fail count and skip
    push ax
    mov ax, 0x0010
    mov ds, ax
    inc word [fail_count]
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

    ; Entry 1 (0x08): ring 0 code
    mov word [gdt + 8 + 0], 0xFFFF
    mov ax, [cs_base_lo]
    mov [gdt + 8 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 8 + 4], al
    mov byte [gdt + 8 + 5], 0x9A       ; P=1, DPL=0, code, exec/read

    ; Entry 2 (0x10): ring 0 data
    mov word [gdt + 16 + 0], 0xFFFF
    mov ax, [cs_base_lo]
    mov [gdt + 16 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 16 + 4], al
    mov byte [gdt + 16 + 5], 0x92      ; P=1, DPL=0, data, read/write

    ; Entry 3 (0x18): ring 3 code
    mov word [gdt + 24 + 0], 0xFFFF
    mov ax, [cs_base_lo]
    mov [gdt + 24 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 24 + 4], al
    mov byte [gdt + 24 + 5], 0xFA      ; P=1, DPL=3, code, exec/read

    ; Entry 4 (0x20): ring 3 data
    mov word [gdt + 32 + 0], 0xFFFF
    mov ax, [cs_base_lo]
    mov [gdt + 32 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 32 + 4], al
    mov byte [gdt + 32 + 5], 0xF2      ; P=1, DPL=3, data, read/write

    ; Entry 5 (0x28): ring 3 stack
    mov word [gdt + 40 + 0], 0xFFFF
    mov ax, [cs_base_lo]
    mov [gdt + 40 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 40 + 4], al
    mov byte [gdt + 40 + 5], 0xF2      ; P=1, DPL=3, data, read/write

    ; Entry 6 (0x30): call gate → ring 0 code, DPL=3
    ; DPL=3 so ring 3 code can call through it
    ; Access: P=1, DPL=3, S=0, type=0x04 (286 call gate) → 0xE4
    mov ax, gate_target_r0
    mov [gdt + 48 + 0], ax             ; offset
    mov word [gdt + 48 + 2], 0x0008    ; target selector (ring 0 code)
    mov byte [gdt + 48 + 4], 0x00      ; word count = 0
    mov byte [gdt + 48 + 5], 0xE4      ; P=1, DPL=3, 286 call gate
    mov word [gdt + 48 + 6], 0x0000

    ; Entry 7 (0x38): TSS descriptor (available 286 TSS)
    ; Access: P=1, DPL=0, S=0, type=0x01 → 0x81
    mov word [gdt + 56 + 0], 43        ; limit = 43 (min 286 TSS)
    mov ax, [cs_base_lo]
    add ax, tss_data
    mov [gdt + 56 + 2], ax
    mov al, [cs_base_hi]
    adc al, 0
    mov [gdt + 56 + 4], al
    mov byte [gdt + 56 + 5], 0x81      ; P=1, DPL=0, available 286 TSS

    ; GDTR: limit = 8*8-1 = 63
    mov word [gdtr_value], 0x003F
    mov ax, [cs_base_lo]
    add ax, gdt
    mov [gdtr_value + 2], ax
    mov al, [cs_base_hi]
    adc al, 0
    mov [gdtr_value + 4], al
    ret

;=============================================================================
; Build IDT — #GP handler at INT 0Dh
;=============================================================================
build_idt:
    mov ax, handler_gp
    mov [idt + 0x68 + 0], ax
    mov word [idt + 0x68 + 2], 0x0008  ; ring 0 code selector
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
    ret

;=============================================================================
; Build TSS — 286 TSS is 44 bytes
; We only need to set the ring 0 SS:SP fields.
; 286 TSS layout:
;   +00: back link selector (unused)
;   +02: SP0 (ring 0 stack pointer)
;   +04: SS0 (ring 0 stack segment selector)
;   +06: SP1, +08: SS1, +0A: SP2, +0C: SS2
;   +0E: IP, +10: FLAGS, +12: AX, ... +22: DI
;   +24: ES, +26: CS, +28: SS, +2A: DS
;   +2C: LDTR
;=============================================================================
build_tss:
    ; Ring 0 stack: SS0=0x0010 (ring 0 data), SP0=0xFFC0
    mov word [tss_data + 2], 0xFFC0    ; SP0
    mov word [tss_data + 4], 0x0010    ; SS0
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
gdt:            resb 64        ; 8 entries * 8
gdtr_value:     resb 6
idt:            resb 0x70      ; covers INT 0x00–0x0D
idtr_value:     resb 6
tss_data:       resb 44        ; 286 TSS
gate_r0_flag:   resw 1
test3_byte:     resb 1
