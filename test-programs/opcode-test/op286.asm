[CPU 286]
[ORG 0x100]

; 286 Opcode Test Suite
; Tests all instructions new in 286 not already in op8086.asm,
; plus instructions whose behavior differs between 8086 and 286.
;
; New 286 instructions:
;   PUSH imm16 (0x68), PUSH imm8 (0x6A)
;   PUSHA (0x60), POPA (0x61)
;   BOUND (0x62)
;   IMUL r16, r/m16, imm16 (0x69)
;   IMUL r16, r/m16, imm8 (0x6B)
;   INSB (0x6C), INSW (0x6D)
;   OUTSB (0x6E), OUTSW (0x6F)
;   SHL/SHR/SAR/ROL/ROR/RCL/RCR r/m8, imm8 (0xC0)
;   SHL/SHR/SAR/ROL/ROR/RCL/RCR r/m16, imm8 (0xC1)
;   ENTER (0xC8), LEAVE (0xC9)
;
; Different 286 behavior:
;   PUSH SP: 8086 pushes SP-2, 286 pushes original SP

section .text
start:
    ; Initialize COM1 serial port
    mov dx, 0          ; COM1
    mov al, 0xE3       ; 9600 baud, no parity, 1 stop, 8 data
    mov ah, 0          ; Initialize serial port
    int 14h

    ; Print banner
    mov si, msg_banner
    call print_string

    ; Initialize test counters
    mov word [pass_count], 0
    mov word [fail_count], 0

    ; 286 behavior difference
    call test_push_sp_286

    ; New stack instructions
    call test_push_imm
    call test_pusha_popa

    ; New arithmetic
    call test_imul_3op

    ; Array bounds check
    call test_bound

    ; Stack frame instructions
    call test_enter_leave

    ; Shift/rotate by immediate count (0xC0/0xC1 encoding - new in 286)
    call test_shl_imm
    call test_shr_imm
    call test_sar_imm
    call test_rol_imm
    call test_ror_imm
    call test_rcl_imm
    call test_rcr_imm

    ; String I/O instructions
    call test_insb
    call test_insw
    call test_outsb
    call test_outsw

    ; Print summary
    call print_summary

    ; Exit to DOS
    mov ax, 0x4C00
    int 21h

;=============================================================================
; Test: PUSH SP (286 behavior: pushes original SP, not SP-2)
; On 8086: PUSH SP pushes the decremented value (SP after push)
; On 286+: PUSH SP pushes the original value (SP before push)
;=============================================================================
test_push_sp_286:
    mov si, test_push_sp_286_name
    call print_test_name

    ; Save SP before push
    mov bx, sp
    push sp
    pop ax
    ; On 286: AX should equal BX (original SP)
    ; On 8086: AX would equal BX-2 (decremented SP)
    cmp ax, bx
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_push_sp_286_failed
    call print_fail
    ret

;=============================================================================
; Test: PUSH imm16 (0x68) and PUSH imm8 sign-extended (0x6A)
; New in 286: push an immediate value directly without a register
;=============================================================================
test_push_imm:
    mov si, test_push_imm_name
    call print_test_name

    ; Test 1: PUSH imm16
    push 0x1234
    pop ax
    cmp ax, 0x1234
    jne .fail

    ; Test 2: PUSH imm16 with large value
    push 0xABCD
    pop bx
    cmp bx, 0xABCD
    jne .fail

    ; Test 3: PUSH imm8 sign-extended (positive, 0x6A encoding)
    push byte 5          ; Sign-extended: 0x0005
    pop cx
    cmp cx, 0x0005
    jne .fail

    ; Test 4: PUSH imm8 sign-extended (negative, sign extends to 0xFFxx)
    push byte -1         ; Sign-extended: 0xFFFF
    pop dx
    cmp dx, 0xFFFF
    jne .fail

    ; Test 5: PUSH imm8 sign-extended (0x7F -> 0x007F, positive)
    push byte 127        ; 0x7F -> 0x007F
    pop ax
    cmp ax, 0x007F
    jne .fail

    ; Test 6: PUSH imm8 sign-extended (0x80 -> 0xFF80, negative)
    push byte -128       ; 0x80 -> 0xFF80
    pop ax
    cmp ax, 0xFF80
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_push_imm_failed
    call print_fail
    ret

;=============================================================================
; Test: PUSHA (0x60) and POPA (0x61)
; PUSHA pushes AX, CX, DX, BX, original SP, BP, SI, DI (in that order)
; POPA pops DI, SI, BP, (skip SP), BX, DX, CX, AX
;=============================================================================
test_pusha_popa:
    mov si, test_pusha_popa_name
    call print_test_name

    ; Set known values in all registers
    mov ax, 0x1111
    mov cx, 0x2222
    mov dx, 0x3333
    mov bx, 0x4444
    ; SP is whatever it is (will be pushed as original value)
    mov bp, 0x5555
    mov si, 0x6666
    mov di, 0x7777

    ; Save SP before PUSHA to verify it was pushed correctly
    mov word [saved_sp], sp

    pusha

    ; Verify SP decreased by 16 (8 words pushed)
    mov ax, [saved_sp]
    sub ax, 16
    cmp sp, ax
    jne .fail

    ; Use BP as frame pointer to inspect pushed values on the stack
    ; Stack layout (top to bottom): DI, SI, BP_old, SP_orig, BX, DX, CX, AX
    ; [bp+0]=DI, [bp+2]=SI, [bp+4]=BP_old, [bp+6]=SP_orig, [bp+8]=BX, [bp+10]=DX, [bp+12]=CX, [bp+14]=AX
    mov bp, sp

    ; Verify the pushed SP value: should equal original SP (pre-PUSHA)
    mov ax, [saved_sp]
    cmp word [bp+6], ax      ; Pushed SP should equal original SP
    jne .fail

    ; Verify AX was pushed correctly (bottom of stack = [bp+14])
    cmp word [bp+14], 0x1111
    jne .fail

    ; Verify CX was pushed correctly
    cmp word [bp+12], 0x2222
    jne .fail

    ; Verify DI was pushed correctly (top of stack = [bp+0])
    cmp word [bp+0], 0x7777
    jne .fail

    ; Now clear registers and POPA to restore
    xor ax, ax
    xor cx, cx
    xor dx, dx
    xor bx, bx
    xor bp, bp
    xor si, si
    xor di, di

    popa

    ; Verify registers restored
    cmp ax, 0x1111
    jne .fail
    cmp cx, 0x2222
    jne .fail
    cmp dx, 0x3333
    jne .fail
    cmp bx, 0x4444
    jne .fail
    cmp bp, 0x5555
    jne .fail
    cmp si, 0x6666
    jne .fail
    cmp di, 0x7777
    jne .fail

    ; Verify SP is restored to original
    cmp sp, [saved_sp]
    jne .fail

    call print_pass
    ret
.fail:
    ; Restore stack in case of failure mid-test
    mov sp, [saved_sp]
    mov si, msg_pusha_popa_failed
    call print_fail
    ret

;=============================================================================
; Test: IMUL 3-operand forms (new in 286)
; IMUL r16, r/m16, imm16 (0x69): dest = src * imm16
; IMUL r16, r/m16, imm8  (0x6B): dest = src * sign_ext(imm8)
;=============================================================================
test_imul_3op:
    mov si, test_imul_3op_name
    call print_test_name

    ; Test 1: IMUL r16, r16, imm16 - basic positive
    mov bx, 5
    imul ax, bx, 7       ; ax = bx * 7 = 35
    cmp ax, 35
    jne .fail

    ; Test 2: IMUL r16, r16, imm16 - source unchanged
    cmp bx, 5            ; BX should be unmodified
    jne .fail

    ; Test 3: IMUL r16, r16, imm16 - negative operand
    mov cx, -3
    imul dx, cx, 4       ; dx = -3 * 4 = -12
    cmp dx, -12
    jne .fail

    ; Test 4: IMUL r16, r16, imm16 - both negative
    mov bx, -6
    imul ax, bx, -5      ; ax = -6 * -5 = 30
    cmp ax, 30
    jne .fail

    ; Test 5: IMUL r16, r16, imm16 - zero
    mov cx, 100
    imul ax, cx, 0       ; ax = 100 * 0 = 0
    cmp ax, 0
    jne .fail

    ; Test 6: IMUL r16, r16, imm16 - large value
    mov bx, 100
    imul ax, bx, 200     ; ax = 100 * 200 = 20000
    cmp ax, 20000
    jne .fail

    ; Test 7: IMUL r16, mem16, imm16
    mov word [test_word], 12
    imul cx, [test_word], 3   ; cx = 12 * 3 = 36
    cmp cx, 36
    jne .fail

    ; Test 8: IMUL r16, r16, imm8 (sign-extended) - 0x6B form
    mov bx, 10
    imul ax, bx, 6       ; ax = 10 * 6 = 60 (imm8 form if count fits in byte)
    cmp ax, 60
    jne .fail

    ; Test 9: IMUL r16, r16, imm8 - negative imm8
    mov cx, 8
    imul dx, cx, -2      ; dx = 8 * -2 = -16
    cmp dx, -16
    jne .fail

    ; Test 10: IMUL r16, r16, imm8 - result is destination only (OF/CF if overflow)
    mov bx, 1000
    imul ax, bx, 100     ; ax = 1000 * 100 = 100000, overflows 16-bit
    ; Overflow occurs, OF and CF should be set
    ; We just verify the truncated result: 100000 & 0xFFFF = 0x86A0
    cmp ax, 0x86A0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_imul_3op_failed
    call print_fail
    ret

;=============================================================================
; Test: BOUND (0x62) - Check array index is within bounds
; BOUND r16, m16:m16 checks: lower_bound <= reg <= upper_bound
; If in bounds: continue; if out of bounds: INT 5
;=============================================================================
test_bound:
    mov si, test_bound_name
    call print_test_name

    ; Set up bounds: lower=0, upper=9 (array of 10 elements)
    mov word [bound_lower], 0
    mov word [bound_upper], 9

    ; Test 1: Index 0 (at lower bound) - should NOT trigger INT 5
    mov ax, 0
    bound ax, [bound_lower]
    ; If we get here, no exception was raised (in bounds)

    ; Test 2: Index 5 (middle) - in bounds
    mov ax, 5
    bound ax, [bound_lower]

    ; Test 3: Index 9 (at upper bound) - in bounds
    mov ax, 9
    bound ax, [bound_lower]

    ; Test 4: Verify bounds are unchanged after BOUND
    cmp word [bound_lower], 0
    jne .fail
    cmp word [bound_upper], 9
    jne .fail

    ; Test 5: Verify register unchanged after successful BOUND
    cmp ax, 9
    jne .fail

    ; Note: Out-of-bounds case (ax < 0 or ax > 9) would trigger INT 5
    ; which would require a custom INT 5 handler. We only test in-bounds here.

    call print_pass
    ret
.fail:
    mov si, msg_bound_failed
    call print_fail
    ret

;=============================================================================
; Test: ENTER (0xC8) and LEAVE (0xC9)
; ENTER imm16, 0 (level=0): PUSH BP; MOV BP,SP; SUB SP,imm16
; LEAVE:                     MOV SP,BP; POP BP
;=============================================================================
test_enter_leave:
    mov si, test_enter_leave_name
    call print_test_name

    ; Test 1: ENTER 4, 0 (allocate 4 bytes, nesting level 0)
    mov bp, 0           ; Clear BP so we can detect it changed
    mov word [saved_sp], sp

    ; Test 1: ENTER 4, 0 - allocate 4 bytes
    call .frame_test_4
    cmp sp, [saved_sp]
    jne .fail

    ; Test 2: ENTER 0, 0 (no locals, just saves BP)
    call .frame_test_nolocals
    cmp sp, [saved_sp]
    jne .fail

    ; Test 3: ENTER with larger allocation
    call .frame_test_large
    cmp sp, [saved_sp]
    jne .fail

    call print_pass
    ret

; ENTER already pushes BP - do NOT add an extra push bp before enter!
.frame_test_4:
    enter 4, 0         ; PUSH BP, MOV BP=SP, SUB SP,4
    ; Verify: SP should equal BP-4
    mov ax, bp
    sub ax, 4
    cmp sp, ax
    jne .inner_fail_4
    leave              ; MOV SP=BP, POP BP — undoes ENTER
    ret
.inner_fail_4:
    leave              ; Clean up ENTER before jumping out
    jmp .fail

.frame_test_nolocals:
    enter 0, 0         ; PUSH BP, MOV BP=SP (no allocation)
    ; SP should equal BP (no locals)
    cmp sp, bp
    jne .inner_fail_nolocals
    leave
    ret
.inner_fail_nolocals:
    leave
    jmp .fail

.frame_test_large:
    enter 16, 0        ; PUSH BP, MOV BP=SP, SUB SP,16
    mov ax, bp
    sub ax, 16
    cmp sp, ax
    jne .inner_fail_large
    leave
    ret
.inner_fail_large:
    leave
    jmp .fail

.fail:
    mov sp, [saved_sp]
    mov si, msg_enter_leave_failed
    call print_fail
    ret

;=============================================================================
; Test: SHL r/m16, imm8 (0xC1 /4) - Shift left by immediate count
; New in 286: shift by any immediate (8086 only had shift by 1 or CL)
;=============================================================================
test_shl_imm:
    mov si, test_shl_imm_name
    call print_test_name

    ; Test 1: SHL r16, imm8 = 1 (same as SHL r,1 but 0xC1 encoding)
    mov ax, 0x0001
    shl ax, 1
    cmp ax, 0x0002
    jne .fail

    ; Test 2: SHL r16, imm8 = 4
    mov ax, 0x0001
    shl ax, 4
    cmp ax, 0x0010
    jne .fail

    ; Test 3: SHL r16, imm8 = 8
    mov bx, 0x0001
    shl bx, 8
    cmp bx, 0x0100
    jne .fail

    ; Test 4: SHL r16, imm8 = 15 (MSB shift)
    mov cx, 0x0001
    shl cx, 15
    cmp cx, 0x8000
    jne .fail

    ; Test 5: SHL r16 shifts out carry
    mov dx, 0x8000
    shl dx, 1
    jnc .fail          ; MSB shifted into carry
    cmp dx, 0
    jne .fail

    ; Test 6: SHL r8, imm8
    mov al, 0x01
    shl al, 3
    cmp al, 0x08
    jne .fail

    ; Test 7: SHL mem16, imm8
    mov word [test_word], 0x0003
    shl word [test_word], 2
    cmp word [test_word], 0x000C
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_shl_imm_failed
    call print_fail
    ret

;=============================================================================
; Test: SHR r/m16, imm8 (0xC1 /5) - Logical shift right by immediate
;=============================================================================
test_shr_imm:
    mov si, test_shr_imm_name
    call print_test_name

    ; Test 1: SHR r16, imm8 = 4
    mov ax, 0x0100
    shr ax, 4
    cmp ax, 0x0010
    jne .fail

    ; Test 2: SHR r16, imm8 = 8
    mov bx, 0x8000
    shr bx, 8
    cmp bx, 0x0080
    jne .fail

    ; Test 3: SHR does NOT sign-extend (logical shift)
    mov cx, 0x8000
    shr cx, 1
    cmp cx, 0x4000       ; MSB = 0, not 1
    jne .fail

    ; Test 4: SHR carry out
    mov dx, 0x0001
    shr dx, 1
    jnc .fail            ; LSB shifts into carry
    cmp dx, 0
    jne .fail

    ; Test 5: SHR r8, imm8
    mov al, 0x80
    shr al, 3
    cmp al, 0x10
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_shr_imm_failed
    call print_fail
    ret

;=============================================================================
; Test: SAR r/m16, imm8 (0xC1 /7) - Arithmetic shift right by immediate
;=============================================================================
test_sar_imm:
    mov si, test_sar_imm_name
    call print_test_name

    ; Test 1: SAR positive value by 4
    mov ax, 0x0100
    sar ax, 4
    cmp ax, 0x0010
    jne .fail

    ; Test 2: SAR negative value by 4 (sign extends)
    mov bx, 0x8000
    sar bx, 4
    cmp bx, 0xF800       ; Sign bit replicated
    jne .fail

    ; Test 3: SAR negative by 8
    mov cx, 0xFF00
    sar cx, 8
    cmp cx, 0xFFFF       ; All sign bits
    jne .fail

    ; Test 4: SAR negative by 1 stays negative
    mov dx, 0x8002
    sar dx, 1
    cmp dx, 0xC001
    jne .fail

    ; Test 5: SAR r8, imm8
    mov al, 0x80        ; -128
    sar al, 2
    cmp al, 0xE0        ; -32 (sign extended)
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_sar_imm_failed
    call print_fail
    ret

;=============================================================================
; Test: ROL r/m16, imm8 (0xC1 /0) - Rotate left by immediate
;=============================================================================
test_rol_imm:
    mov si, test_rol_imm_name
    call print_test_name

    ; Test 1: ROL by 4
    mov ax, 0x1234
    rol ax, 4
    cmp ax, 0x2341
    jne .fail

    ; Test 2: ROL by 8 (byte swap)
    mov bx, 0xABCD
    rol bx, 8
    cmp bx, 0xCDAB
    jne .fail

    ; Test 3: ROL by 1 (same as original but via 0xC1 encoding)
    mov cx, 0x8000
    rol cx, 1
    jnc .fail            ; MSB should be in carry
    cmp cx, 0x0001       ; Bit wrapped to LSB
    jne .fail

    ; Test 4: ROL r8, imm8
    mov al, 0x81
    rol al, 1
    cmp al, 0x03         ; 1000 0001 -> 0000 0011
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_rol_imm_failed
    call print_fail
    ret

;=============================================================================
; Test: ROR r/m16, imm8 (0xC1 /1) - Rotate right by immediate
;=============================================================================
test_ror_imm:
    mov si, test_ror_imm_name
    call print_test_name

    ; Test 1: ROR by 4
    mov ax, 0x1234
    ror ax, 4
    cmp ax, 0x4123
    jne .fail

    ; Test 2: ROR by 8 (byte swap)
    mov bx, 0xABCD
    ror bx, 8
    cmp bx, 0xCDAB
    jne .fail

    ; Test 3: ROR by 1 - LSB wraps to MSB
    mov cx, 0x0001
    ror cx, 1
    jnc .fail            ; LSB should be in carry
    cmp cx, 0x8000       ; Bit wrapped to MSB
    jne .fail

    ; Test 4: ROR r8, imm8
    mov al, 0x81
    ror al, 1
    ; 1000 0001 -> 1100 0000 (carry=1, bit wraps to MSB)
    jnc .fail
    cmp al, 0xC0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_ror_imm_failed
    call print_fail
    ret

;=============================================================================
; Test: RCL r/m16, imm8 (0xC1 /2) - Rotate left through carry by immediate
;=============================================================================
test_rcl_imm:
    mov si, test_rcl_imm_name
    call print_test_name

    ; Test 1: RCL by 2 (rotate two positions through carry)
    clc
    mov ax, 0x0001       ; bit 0 = 1, others = 0
    rcl ax, 2            ; CF(0)->bit0, bit0->bit1, bit15->CF
    ; 0000 0000 0000 0001 RCL 2 with CF=0:
    ; step1: CF=0->bit0, bit0(1)->CF: 0000 0000 0000 0010, CF=0
    ; But wait, RCL with count uses the 17-bit rotation
    ; Actually with count=2: rotate the whole 17-bit value left by 2
    ; 17-bit: CF|AX = 0|0000 0000 0000 0001 = 00000000000000001
    ; Rotate 2 left: 000000000000000100 -> CF=0, AX=0x0004
    cmp ax, 0x0004
    jne .fail
    jc .fail             ; CF should be 0

    ; Test 2: RCL by 2 with carry set
    stc
    mov bx, 0x0001
    rcl bx, 2            ; 17-bit: 1|0000 0000 0000 0001 = 10000000000000001
    ; Rotate left by 2: 000000000000001 10 -> CF = 0, BX = 0x0006... wait
    ; Let me recalculate: 17-bit value = 1_0000_0000_0000_0001 (CF=1 in bit16)
    ; Rotate left by 2: move top 2 bits to bottom 2
    ; Result: 0_0000_0000_0000_0110 with CF from original bit15 = 0
    ; Wait: 17 bits: bit16=CF=1, bits15-0=0x0001
    ; Shift left by 2: the 17-bit value becomes the old bits 14-0 at positions 0-14, old bit15 at pos 15, old bit16 at...
    ; Actually: rotate left means: new[16..0] = old[14..0, 16, 15]... No.
    ; RCL count: rotate left through carry, count times.
    ; Each step: new_CF = old_bit15, new_bit15..1 = old_bit14..0, new_bit0 = old_CF
    ; Step 1 with CF=1, AX=0x0001: new_CF=0, AX=0x0003
    ; Step 2 with CF=0, AX=0x0003: new_CF=0, AX=0x0006
    cmp bx, 0x0006
    jne .fail
    jc .fail             ; CF should be 0 after step 2

    ; Test 3: RCL by 4 - verify carry propagates correctly
    clc
    mov cx, 0x8000
    rcl cx, 4            ; Bit 15 rotates through CF after 1 step
    ; Step1: CF=1, CX=0x0000
    ; Step2: CF=0, CX=0x0001
    ; Step3: CF=0, CX=0x0002
    ; Step4: CF=0, CX=0x0004
    cmp cx, 0x0004
    jne .fail
    jc .fail

    call print_pass
    ret
.fail:
    mov si, msg_rcl_imm_failed
    call print_fail
    ret

;=============================================================================
; Test: RCR r/m16, imm8 (0xC1 /3) - Rotate right through carry by immediate
;=============================================================================
test_rcr_imm:
    mov si, test_rcr_imm_name
    call print_test_name

    ; Test 1: RCR by 2 with CF=0
    clc
    mov ax, 0x0004       ; bit 2 = 1
    rcr ax, 2
    ; 17-bit: CF=0, AX=0x0004 = 0_0000_0000_0000_0100
    ; Rotate right 2: 00_0000_0000_0000_01 -> CF=0, AX=0x0001
    cmp ax, 0x0001
    jne .fail
    jc .fail

    ; Test 2: RCR by 2 with CF=1
    stc
    mov bx, 0x0000
    rcr bx, 2
    ; 17-bit: CF=1, BX=0x0000 = 1_0000_0000_0000_0000
    ; Rotate right 2: the 17-bit value right 2:
    ; Step1: new_bit15=old_CF=1, new_bits14-0=old_bits15-1, new_CF=old_bit0=0
    ;        CF=0, BX=0x8000
    ; Step2: new_bit15=old_CF=0, new_bits14-0=old_bits15-1(=0x4000), new_CF=old_bit0=0
    ;        CF=0, BX=0x4000
    cmp bx, 0x4000
    jne .fail
    jc .fail

    ; Test 3: RCR by 4 - bit 0 ends up in carry
    clc
    mov cx, 0x0010
    rcr cx, 5            ; Rotate right 5: bit 4 ends up at bit-1 = CF
    ; 17-bit: 0_0000_0000_0001_0000 rotate right 5
    ; = 1000_0_0000_0000_0001 >> 0 -> CF = old bit 4 = 1... let me compute differently
    ; Actually bit 4 of 0x0010 is at position 4. Rotating 17-bit right by 5:
    ; Bit 4 -> Bit -1 which wraps to bit 16 = CF.
    ; So after rotating right 5, original bit 4 is in CF.
    jnc .fail            ; Bit 4 should be in CF

    call print_pass
    ret
.fail:
    mov si, msg_rcr_imm_failed
    call print_fail
    ret

;=============================================================================
; Test: INSB (0x6C) - Input byte from port DX into ES:DI
; Reads a byte from I/O port DX into memory at ES:DI, then increments DI
;=============================================================================
test_insb:
    mov si, test_insb_name
    call print_test_name

    ; Set up: read from port 0x61 (keyboard/speaker control - always readable)
    ; Write result to test_buffer
    mov dx, 0x61         ; Port 0x61 - keyboard controller port B
    mov di, test_buffer  ; ES:DI -> test_buffer (ES=DS in .COM)
    cld                  ; Forward direction

    ; Clear buffer first
    mov byte [test_buffer], 0xFF

    ; Execute INSB
    insb                 ; Read byte from port DX into [ES:DI], DI++

    ; Verify DI was incremented by 1
    cmp di, test_buffer + 1
    jne .fail

    ; Test with STD (decrement direction)
    mov di, test_buffer + 1
    std
    insb                 ; DI should decrement
    cmp di, test_buffer
    jne .fail_restore_df

    cld                  ; Restore DF

    ; Test REP INSB
    mov cx, 4
    mov di, test_buffer
    mov dx, 0x61
    cld
    rep insb
    cmp cx, 0            ; CX exhausted
    jne .fail
    cmp di, test_buffer + 4   ; DI advanced by 4
    jne .fail

    call print_pass
    ret
.fail_restore_df:
    cld
.fail:
    cld
    mov si, msg_insb_failed
    call print_fail
    ret

;=============================================================================
; Test: INSW (0x6D) - Input word from port DX into ES:DI
;=============================================================================
test_insw:
    mov si, test_insw_name
    call print_test_name

    ; Read from port 0x40 (PIT channel 0 counter - returns latched value)
    mov dx, 0x40
    mov di, word_buffer
    cld

    insw                 ; Read word from port DX into [ES:DI], DI += 2

    ; Verify DI was incremented by 2
    cmp di, word_buffer + 2
    jne .fail

    ; Test with STD (decrement)
    mov di, word_buffer + 2
    std
    insw
    cmp di, word_buffer
    jne .fail_restore_df

    cld

    ; Test REP INSW
    mov cx, 3
    mov di, word_buffer
    mov dx, 0x40
    cld
    rep insw
    cmp cx, 0
    jne .fail
    cmp di, word_buffer + 6
    jne .fail

    call print_pass
    ret
.fail_restore_df:
    cld
.fail:
    cld
    mov si, msg_insw_failed
    call print_fail
    ret

;=============================================================================
; Test: OUTSB (0x6E) - Output byte from DS:SI to port DX
;=============================================================================
test_outsb:
    mov si, test_outsb_name
    call print_test_name

    ; Set up: write to port 0x80 (POST diagnostic port - safe to write)
    mov byte [test_buffer], 0xAA
    mov si, test_buffer
    mov dx, 0x80         ; POST port - safe for writes
    cld

    ; Save SI position
    mov bx, si

    outsb                ; Write byte from [DS:SI] to port DX, SI++

    ; Verify SI was incremented by 1
    inc bx
    cmp si, bx
    jne .fail

    ; Test with STD (decrement)
    mov si, test_buffer + 1
    std
    outsb
    cmp si, test_buffer
    jne .fail_restore_df

    cld

    ; Test REP OUTSB
    mov byte [test_buffer],   0x11
    mov byte [test_buffer+1], 0x22
    mov byte [test_buffer+2], 0x33
    mov cx, 3
    mov si, test_buffer
    mov dx, 0x80
    cld
    rep outsb
    cmp cx, 0
    jne .fail
    cmp si, test_buffer + 3
    jne .fail

    call print_pass
    ret
.fail_restore_df:
    cld
.fail:
    cld
    mov si, msg_outsb_failed
    call print_fail
    ret

;=============================================================================
; Test: OUTSW (0x6F) - Output word from DS:SI to port DX
;=============================================================================
test_outsw:
    mov si, test_outsw_name
    call print_test_name

    ; Write to port 0x80 (POST port)
    mov word [word_buffer], 0x1234
    mov si, word_buffer
    mov dx, 0x80
    cld

    mov bx, si

    outsw                ; Write word from [DS:SI] to port DX, SI += 2

    ; Verify SI was incremented by 2
    add bx, 2
    cmp si, bx
    jne .fail

    ; Test with STD (decrement)
    mov si, word_buffer + 2
    std
    outsw
    cmp si, word_buffer
    jne .fail_restore_df

    cld

    ; Test REP OUTSW
    mov word [word_buffer],   0xAAAA
    mov word [word_buffer+2], 0xBBBB
    mov word [word_buffer+4], 0xCCCC
    mov cx, 3
    mov si, word_buffer
    mov dx, 0x80
    cld
    rep outsw
    cmp cx, 0
    jne .fail
    cmp si, word_buffer + 6
    jne .fail

    call print_pass
    ret
.fail_restore_df:
    cld
.fail:
    cld
    mov si, msg_outsw_failed
    call print_fail
    ret

;=============================================================================
; Helper: Print test name
;=============================================================================
print_test_name:
    call print_string
    mov al, ':'
    call print_char
    mov al, ' '
    call print_char
    ret

;=============================================================================
; Helper: Print PASS
;=============================================================================
print_pass:
    mov si, msg_pass
    call print_string
    inc word [pass_count]
    ret

;=============================================================================
; Helper: Print FAIL with message
;=============================================================================
print_fail:
    push si
    mov si, msg_fail
    call print_string
    pop si
    call print_string
    call print_newline
    inc word [fail_count]
    ret

;=============================================================================
; Helper: Print summary
;=============================================================================
print_summary:
    call print_newline
    mov si, msg_summary
    call print_string

    ; Print pass count
    mov ax, [pass_count]
    call print_number
    mov si, msg_passed
    call print_string

    ; Print fail count
    mov ax, [fail_count]
    call print_number
    mov si, msg_failed
    call print_string

    call print_newline
    ret

;=============================================================================
; Helper: Print string (SI points to null-terminated string)
;=============================================================================
print_string:
    push ax
    push dx
    mov dx, 0          ; COM1
.loop:
    lodsb
    test al, al
    jz .done
    mov ah, 1          ; Write character
    int 14h
    jmp .loop
.done:
    pop dx
    pop ax
    ret

;=============================================================================
; Helper: Print character (AL = character)
;=============================================================================
print_char:
    push ax
    push dx
    mov dx, 0          ; COM1
    mov ah, 1          ; Write character
    int 14h
    pop dx
    pop ax
    ret

;=============================================================================
; Helper: Print newline
;=============================================================================
print_newline:
    push ax
    mov al, 13         ; CR
    call print_char
    mov al, 10         ; LF
    call print_char
    pop ax
    ret

;=============================================================================
; Helper: Print number in AX as decimal
;=============================================================================
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

msg_banner: db '=== oxide86 286 Opcode Test Suite ===', 13, 10, 0

test_push_sp_286_name:  db 'PUSH SP (286)', 0
test_push_imm_name:     db 'PUSH imm', 0
test_pusha_popa_name:   db 'PUSHA/POPA', 0
test_imul_3op_name:     db 'IMUL 3-op', 0
test_bound_name:        db 'BOUND', 0
test_enter_leave_name:  db 'ENTER/LEAVE', 0
test_shl_imm_name:      db 'SHL imm', 0
test_shr_imm_name:      db 'SHR imm', 0
test_sar_imm_name:      db 'SAR imm', 0
test_rol_imm_name:      db 'ROL imm', 0
test_ror_imm_name:      db 'ROR imm', 0
test_rcl_imm_name:      db 'RCL imm', 0
test_rcr_imm_name:      db 'RCR imm', 0
test_insb_name:         db 'INSB', 0
test_insw_name:         db 'INSW', 0
test_outsb_name:        db 'OUTSB', 0
test_outsw_name:        db 'OUTSW', 0

msg_pass: db 'PASS', 13, 10, 0
msg_fail: db 'FAIL - ', 0

msg_push_sp_286_failed:  db 'PUSH SP should push original SP on 286', 0
msg_push_imm_failed:     db 'immediate push value incorrect', 0
msg_pusha_popa_failed:   db 'register save/restore incorrect', 0
msg_imul_3op_failed:     db '3-operand IMUL result incorrect', 0
msg_bound_failed:        db 'BOUND check failed', 0
msg_enter_leave_failed:  db 'stack frame setup/teardown incorrect', 0
msg_shl_imm_failed:      db 'SHL by immediate result incorrect', 0
msg_shr_imm_failed:      db 'SHR by immediate result incorrect', 0
msg_sar_imm_failed:      db 'SAR by immediate result incorrect', 0
msg_rol_imm_failed:      db 'ROL by immediate result incorrect', 0
msg_ror_imm_failed:      db 'ROR by immediate result incorrect', 0
msg_rcl_imm_failed:      db 'RCL by immediate result incorrect', 0
msg_rcr_imm_failed:      db 'RCR by immediate result incorrect', 0
msg_insb_failed:         db 'INSB pointer adjustment incorrect', 0
msg_insw_failed:         db 'INSW pointer adjustment incorrect', 0
msg_outsb_failed:        db 'OUTSB pointer adjustment incorrect', 0
msg_outsw_failed:        db 'OUTSW pointer adjustment incorrect', 0

msg_summary: db '--- Summary ---', 13, 10, 0
msg_passed:  db ' passed, ', 0
msg_failed:  db ' failed', 0

section .bss
pass_count:  resw 1
fail_count:  resw 1
test_buffer: resb 16
test_word:   resw 1
bound_lower: resw 1
bound_upper: resw 1
saved_sp:    resw 1
word_buffer: resw 10
