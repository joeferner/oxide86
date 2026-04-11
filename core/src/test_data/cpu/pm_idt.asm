[CPU 286]
[ORG 0x100]

; Protected Mode Step 5: IDT-based Interrupt Dispatch
;
; Tests that in protected mode, INT instructions dispatch through the IDT
; (Interrupt Descriptor Table) instead of the real-mode IVT.
;
; GDT layout:
;   0x00: null descriptor
;   0x08: code segment — base=CS<<4, limit=0xFFFF, exec/read, DPL 0
;   0x10: data segment — base=CS<<4, limit=0xFFFF, read/write, DPL 0
;
; IDT layout:
;   Entry 0x20 (INT 20h): interrupt gate → 0x0008:handler_int20
;   Entry 0x21 (INT 21h): trap gate     → 0x0008:handler_int21
;
; Tests:
;   1. INT 20h through interrupt gate — handler runs, sets a flag, IRETs back
;   2. INT 20h clears IF (interrupt gate behavior)
;   3. INT 21h through trap gate — handler runs, sets a flag, IRETs back
;   4. INT 21h preserves IF (trap gate behavior)
;   5. IRET from PM handler restores FLAGS, CS, IP correctly

section .text
start:
    ; === Real-mode setup ===
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

    ; Enable interrupts so we can test IF behavior
    sti

    ; --- Test 1: INT 20h dispatches through IDT interrupt gate ---
    mov byte [int20_flag], 0
    int 0x20
    cmp byte [int20_flag], 0xAA
    jne .test1_fail
    inc word [pass_count]
    jmp .test2
.test1_fail:
    inc word [fail_count]

.test2:
    ; --- Test 2: INT 20h (interrupt gate) clears IF ---
    ; The handler stores IF state before IRET.
    ; An interrupt gate should clear IF before entering the handler.
    cmp byte [int20_if_state], 0
    je .test2_pass
    inc word [fail_count]
    jmp .test3
.test2_pass:
    inc word [pass_count]

.test3:
    ; --- Test 3: INT 21h dispatches through IDT trap gate ---
    sti                         ; re-enable IF
    mov byte [int21_flag], 0
    int 0x21
    cmp byte [int21_flag], 0xBB
    jne .test3_fail
    inc word [pass_count]
    jmp .test4
.test3_fail:
    inc word [fail_count]

.test4:
    ; --- Test 4: INT 21h (trap gate) preserves IF ---
    ; Trap gate should NOT clear IF.
    cmp byte [int21_if_state], 1
    je .test4_pass
    inc word [fail_count]
    jmp .test5
.test4_pass:
    inc word [pass_count]

.test5:
    ; --- Test 5: IRET restores FLAGS correctly ---
    ; Set CF=1 before INT, verify it's preserved after IRET
    stc                         ; set carry flag
    sti                         ; ensure IF=1
    int 0x20
    jnc .test5_fail             ; CF should still be set after IRET
    inc word [pass_count]
    jmp .done
.test5_fail:
    inc word [fail_count]

.done:
    ; We can't use INT 21h to exit because our IDT overrides INT 21h
    ; with a test handler. Instead, restore the real-mode IDTR and
    ; clear PE so INT 21h dispatches through the IVT to the BIOS.
    ;
    ; On a real 286, clearing PE requires a CPU reset. But the emulator's
    ; LMSW prevents clearing PE. So we load the real-mode IDTR (which
    ; makes IDT lookup fall through to real-mode-compatible addresses)
    ; and then set up a special IDT entry for INT 21h that points to
    ; the BIOS handler.
    ;
    ; Simplest approach: just set IDTR to the real-mode IVT layout.
    ; The IVT has 4-byte entries, but the IDT expects 8-byte gates.
    ; Reading INT 21h (offset 0x108) from the IVT area gives garbage.
    ;
    ; Real solution: set IDTR limit to 0 so all IDT lookups fail,
    ; and rely on the fallback to real-mode IVT in dispatch_interrupt_pm.
    lidt [empty_idtr]
    mov ah, 4Ch
    mov al, [fail_count]
    int 21h

;=============================================================================
; INT 20h handler (interrupt gate — should have IF=0 on entry)
;=============================================================================
handler_int20:
    mov byte [int20_flag], 0xAA
    ; Record IF state: read FLAGS from stack (SP+4) or use pushf
    pushf
    pop ax
    and ax, 0x0200              ; isolate IF bit
    shr ax, 9                   ; 0 or 1
    mov [int20_if_state], al
    iret

;=============================================================================
; INT 21h handler (trap gate — should have IF unchanged on entry)
;=============================================================================
handler_int21:
    mov byte [int21_flag], 0xBB
    ; Record IF state
    pushf
    pop ax
    and ax, 0x0200
    shr ax, 9
    mov [int21_if_state], al
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
    mov byte [gdt + 8 + 5], 0x9A  ; present, DPL0, code, exec/read

    ; Entry 2 (0x10): data segment
    mov word [gdt + 16 + 0], 0xFFFF
    mov ax, [cs_base_lo]
    mov [gdt + 16 + 2], ax
    mov al, [cs_base_hi]
    mov [gdt + 16 + 4], al
    mov byte [gdt + 16 + 5], 0x92  ; present, DPL0, data, read/write

    ; GDTR
    mov word [gdtr_value], 0x0017  ; limit = 3*8 - 1
    mov ax, [cs_base_lo]
    add ax, gdt
    mov [gdtr_value + 2], ax
    mov al, [cs_base_hi]
    adc al, 0
    mov [gdtr_value + 4], al
    ret

;=============================================================================
; Build IDT
; 286 IDT entry (8 bytes):
;   bytes 0-1: offset (low 16 bits of handler)
;   bytes 2-3: selector (code segment selector)
;   byte  4:   reserved (0)
;   byte  5:   type/attr: P(1) | DPL(2) | 0(1) | TYPE(4)
;              Interrupt gate 286: type = 0x06 → attr = 0x86 (P=1,DPL=0,type=6)
;              Trap gate 286:      type = 0x07 → attr = 0x87 (P=1,DPL=0,type=7)
;   bytes 6-7: reserved (0)
;=============================================================================
build_idt:
    ; We need entries for INT 20h and INT 21h.
    ; INT 20h is at IDT offset 0x20 * 8 = 0x100
    ; INT 21h is at IDT offset 0x21 * 8 = 0x108
    ; So the IDT needs to be at least 0x110 bytes (limit = 0x10F)

    ; IDT entry for INT 20h (offset 0x100): interrupt gate
    mov ax, handler_int20       ; handler offset within code segment
    mov [idt + 0x100 + 0], ax   ; offset
    mov word [idt + 0x100 + 2], 0x0008  ; selector = code segment
    mov byte [idt + 0x100 + 4], 0x00    ; reserved
    mov byte [idt + 0x100 + 5], 0x86    ; P=1, DPL=0, 286 interrupt gate
    mov word [idt + 0x100 + 6], 0x0000  ; reserved

    ; IDT entry for INT 21h (offset 0x108): trap gate
    mov ax, handler_int21
    mov [idt + 0x108 + 0], ax
    mov word [idt + 0x108 + 2], 0x0008
    mov byte [idt + 0x108 + 4], 0x00
    mov byte [idt + 0x108 + 5], 0x87    ; P=1, DPL=0, 286 trap gate
    mov word [idt + 0x108 + 6], 0x0000

    ; IDTR: base = physical address of idt, limit = 0x10F
    mov word [idtr_value], 0x010F
    mov ax, [cs_base_lo]
    add ax, idt
    mov [idtr_value + 2], ax
    mov al, [cs_base_hi]
    adc al, 0
    mov [idtr_value + 4], al

    ; Set up an empty IDTR (limit=0) for exit — forces fallback to real-mode IVT
    mov word [empty_idtr], 0x0000
    mov word [empty_idtr + 2], 0x0000
    mov word [empty_idtr + 4], 0x0000

    ret

;=============================================================================
; Helpers
;=============================================================================

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

msg_summary: db '--- Summary ---', 13, 10, 0
msg_passed:  db ' passed, ', 0
msg_failed:  db ' failed', 0

; Empty IDTR (for exit — forces fallback to real-mode IVT)
empty_idtr:
    dw 0x0000
    db 0x00, 0x00, 0x00
    db 0x00

section .bss
pass_count:     resw 1
fail_count:     resw 1
cs_base_lo:     resw 1
cs_base_hi:     resb 1
gdt:            resb 24        ; 3 entries * 8 bytes
gdtr_value:     resb 6
idt:            resb 0x110     ; enough for INT 0x00–0x21 (0x22 entries * 8)
idtr_value:     resb 6
int20_flag:     resb 1
int20_if_state: resb 1
int21_flag:     resb 1
int21_if_state: resb 1
