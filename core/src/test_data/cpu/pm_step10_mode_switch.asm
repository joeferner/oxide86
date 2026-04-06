[CPU 286]
[ORG 0x100]

; Protected Mode Step 10: Real Mode → Protected Mode Transition and Reset Path
;
; Tests:
;   1. On startup, PE=0 (real mode) — verified by SMSW
;   2. After LMSW sets PE, SMSW confirms PE=1
;   3. Real-mode programs work correctly with CPU type 286 and PE=0
;      (verified by the fact that all the setup code runs in real mode)
;   4. Keyboard controller reset command (0xFE on port 0x64) triggers CPU reset
;      — the program restarts and PE=0 again
;   5. After reset, the program runs in real mode correctly
;
; Strategy:
;   The program uses a flag at a fixed physical address (0x00500, in the
;   BDA scratch area) to detect whether this is the first or second run.
;   On first run: set flag, enter PM, trigger reset.
;   On second run: flag is 0 (memory cleared), verify PE=0, exit.
;
;   Actually, Computer::reset() clears memory, so we can't use a flag.
;   Instead: the program always starts by checking PE. If PE=0, it's either
;   the first run or a post-reset run. We use a counter approach:
;   - First run: check PE=0 (pass), enter PM, do work, trigger reset
;   - Second run: check PE=0 (pass), try to exit immediately
;   - To distinguish: we pass information via a mechanism that survives reset.
;
;   Simplest: just test that PE=0 at startup, enter PM, trigger reset via
;   the keyboard controller. If the reset works, Computer::reset() reloads
;   the program and it starts again with PE=0. The second time it reaches
;   the SMSW check, PE is 0 again, and it exits. But how to prevent
;   an infinite loop of reset-restart-reset?
;
;   Solution: use the CMOS shutdown byte (register 0x0F). Before reset,
;   write a non-zero shutdown code. After reset, the BIOS checks CMOS 0x0F.
;   If it's non-zero, it should jump to the address in BDA 40:67h instead
;   of running the normal boot sequence.
;
;   For the test framework (load_program), there's no BIOS boot sequence —
;   the program just starts. So the CMOS approach doesn't help.
;
;   Practical approach: write a recognizable value to CMOS register 0x0F
;   before the reset. After restart, read CMOS 0x0F. If it's 0 (default),
;   this is the first run. If it's non-zero, this is a post-reset run.
;   CMOS is battery-backed and should survive a soft reset.

section .text
start:
    mov word [pass_count], 0
    mov word [fail_count], 0

    ; Read CMOS shutdown byte (register 0x0F) to detect post-reset.
    ; A fresh system should return 0x00 (first run).
    ; After we write 0x05 and trigger reset, it should still be 0x05 (post-reset).
    mov al, 0x0F
    out 0x70, al
    in al, 0x71
    mov [shutdown_byte], al
    cmp al, 0x05                ; our shutdown code from the first run
    je .post_reset

    ; === First run ===

    ; --- Test 1: PE=0 at startup ---
    smsw ax
    test ax, 1
    jnz .test1_fail
    inc word [pass_count]
    jmp .test2
.test1_fail:
    inc word [fail_count]

.test2:
    ; --- Test 2: Enter PM, verify PE=1 ---
    call build_gdt
    lgdt [gdtr_value]
    cli
    mov ax, 0x0001
    lmsw ax

    smsw ax
    test ax, 1
    jz .test2_fail
    inc word [pass_count]
    jmp .test3
.test2_fail:
    inc word [fail_count]

.test3:
    ; --- Test 3: Set up for reset ---
    ; Write shutdown code 0x05 to CMOS register 0x0F
    ; (0x05 = JMP to BDA 40:67h with EOI — standard 286 PM exit code)
    mov al, 0x0F
    out 0x70, al
    mov al, 0x05
    out 0x71, al

    ; Far JMP to load CS with a valid PM selector before triggering reset
    jmp 0x0008:.in_pm

.in_pm:
    mov ax, 0x0010
    mov ds, ax
    mov ss, ax

    ; --- Test 4: Trigger CPU reset via keyboard controller ---
    ; Command 0xFE on port 0x64 = pulse reset line
    mov al, 0xFE
    out 0x64, al

    ; If the reset doesn't happen, we'll reach here (fail)
    ; Wait a few steps for the reset to take effect
    nop
    nop
    nop

    ; If we reach here, reset didn't work
    ; Load empty IDTR and exit with failures
    lidt [empty_idtr]
    mov ah, 4Ch
    mov al, 5
    int 21h

; === Post-reset path ===
.post_reset:
    ; --- Test 5: After reset, PE=0 ---
    smsw ax
    test ax, 1
    jnz .test5_fail
    inc word [pass_count]
    jmp .post_done
.test5_fail:
    inc word [fail_count]

.post_done:
    ; Clear the CMOS shutdown byte so we don't loop
    mov al, 0x0F
    out 0x70, al
    mov al, 0x00
    out 0x71, al

    ; Exit with fail count
    mov ah, 4Ch
    mov al, [fail_count]
    int 21h

;=============================================================================
; Build GDT (minimal, for PM entry)
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

    ; GDTR
    mov word [gdtr_value], 0x0017
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
pass_count:    resw 1
fail_count:    resw 1
shutdown_byte: resb 1
cs_base_lo:    resw 1
cs_base_hi:    resb 1
gdt:           resb 24
gdtr_value:    resb 6
