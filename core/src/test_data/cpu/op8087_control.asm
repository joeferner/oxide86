; op8087_control.asm - 8087 FPU control and state instruction tests
;
; Tests:
;   FINIT   - Initialize (reset) FPU; verifies status word cleared
;   FLDCW / FNSTCW - Load/Store control word round-trip
;   FNSAVE / FRSTOR - Save/Restore full FPU state
;   FXCH    - Exchange ST(0) with ST(i)
;
; Exit codes:
;   0x00 = all tests passed
;   0x01 = one or more tests failed

[CPU 8086]
[ORG 0x100]

section .text
start:
    mov word [pass_count], 0
    mov word [fail_count], 0

    call test_finit
    call test_fldcw_fnstcw
    call test_fnsave_frstor
    call test_fxch

    cmp word [fail_count], 0
    jne .fail

    mov ah, 0x4C
    mov al, 0x00
    int 0x21

.fail:
    mov ah, 0x4C
    mov al, 0x01
    int 0x21

;=============================================================================
; Test: FINIT — initialize (wait + reset) the coprocessor
; After FINIT, the status word must read 0x0000.
;=============================================================================
test_finit:
    fninit
    fld dword [val_1f32]    ; push something so TOP != 0
    finit                   ; wait + reset: status word -> 0x0000, TOP -> 0
    db 0xDF, 0xE0           ; FNSTSW AX
    test ax, ax
    jnz .fail               ; non-zero means FPU was not reset
    inc word [pass_count]
    ret
.fail:
    inc word [fail_count]
    ret

;=============================================================================
; Test: FLDCW / FNSTCW — load a control word, store it back, verify
;=============================================================================
test_fldcw_fnstcw:
    fninit
    fldcw [cw_test]         ; load known control word
    fnstcw [scratch_cw]     ; store it back (no-wait)
    mov si, cw_test
    mov di, scratch_cw
    mov cx, 2
    call compare_bytes
    ret

;=============================================================================
; Test: FNSAVE / FRSTOR — save state, overwrite, restore, verify values
; Load 1.0 onto stack, FNSAVE (saves + resets), load 2.0, FRSTOR,
; then pop ST(0) and verify it is 1.0.
;=============================================================================
test_fnsave_frstor:
    fninit
    fld dword [val_1f32]        ; ST(0) = 1.0
    fnsave [state_buf]          ; save state (also resets FPU)
    fld dword [val_2f32]        ; ST(0) = 2.0 on fresh FPU
    frstor [state_buf]          ; restore: ST(0) should be 1.0 again
    fstp dword [scratch32]      ; pop into scratch
    mov si, val_1f32
    mov di, scratch32
    mov cx, 4
    call compare_bytes
    ret

;=============================================================================
; Test: FXCH — exchange ST(0) with ST(1)
; Load 1.0 (ST(1)) then 2.0 (ST(0)), FXCH, pop both,
; verify ST(0) is now 1.0 and ST(1) is now 2.0.
;=============================================================================
test_fxch:
    fninit
    fld dword [val_1f32]    ; ST(0) = 1.0
    fld dword [val_2f32]    ; ST(0) = 2.0, ST(1) = 1.0
    fxch                    ; ST(0) = 1.0, ST(1) = 2.0
    fstp dword [scratch32]  ; pop ST(0) = 1.0
    mov si, val_1f32
    mov di, scratch32
    mov cx, 4
    call compare_bytes
    fstp dword [scratch32]  ; pop ST(0) = 2.0
    mov si, val_2f32
    mov di, scratch32
    mov cx, 4
    call compare_bytes
    ret

;=============================================================================
; compare_bytes — compare [SI] to [DI] for CX bytes
; Increments pass_count if all match, fail_count if any differ.
;=============================================================================
compare_bytes:
    push si
    push di
    push cx
.loop:
    mov al, [si]
    cmp al, [di]
    jne .fail
    inc si
    inc di
    loop .loop
    inc word [pass_count]
    pop cx
    pop di
    pop si
    ret
.fail:
    pop cx
    pop di
    pop si
    inc word [fail_count]
    ret

;=============================================================================
; Data
;=============================================================================
section .data
val_1f32: dd 0x3F800000     ; 1.0 (IEEE 754 single)
val_2f32: dd 0x40000000     ; 2.0 (IEEE 754 single)
cw_test:  dw 0x027F         ; custom control word (round down, extended precision)

section .bss
scratch32:  resd 1
scratch_cw: resw 1
; FNSAVE state buffer: 14 bytes header + 8 x 10 bytes registers = 94 bytes
state_buf:  resb 94
pass_count: resw 1
fail_count: resw 1
