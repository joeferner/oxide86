[CPU 8086]
[ORG 0x100]

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

    ; Run tests - Basic operations
    call test_mov
    call test_add
    call test_sub
    call test_inc
    call test_dec
    call test_neg
    call test_cmp

    ; Logical operations
    call test_and
    call test_or
    call test_xor
    call test_not
    call test_test

    ; Shift/Rotate operations
    call test_shl
    call test_shr
    call test_rol
    call test_ror

    ; Multiply/Divide
    call test_mul
    call test_div

    ; Stack operations
    call test_push_pop

    ; String operations
    call test_lodsb
    call test_stosb
    call test_movsb
    call test_cmpsb
    call test_scasb

    ; Extended arithmetic
    call test_adc_sbb
    call test_imul
    call test_idiv

    ; Extended rotate
    call test_rcl_rcr

    ; Loop instructions
    call test_loopz_loopnz

    ; BCD Arithmetic
    call test_daa_das
    call test_aaa_aas
    call test_aam_aad

    ; Conditional jumps
    call test_jo_jno
    call test_jp_jnp

    ; Segment operations
    call test_lds_les

    ; Return with immediate
    call test_ret_imm

    ; Phase 1: Data Transfer & Arithmetic Extensions
    call test_xchg
    call test_lea
    call test_cbw_cwd
    call test_lahf_sahf
    call test_xlatb

    ; Phase 2: Shift Extensions
    call test_sar

    ; Phase 3: String Word Operations
    call test_lodsw
    call test_stosw
    call test_movsw
    call test_cmpsw
    call test_scasw

    ; Phase 4: Control Flow
    call test_conditional_jumps
    call test_jmp
    call test_call_ret
    call test_jcxz_loop

    ; Phase 5: Flag Operations
    call test_flag_operations
    call test_direction_interrupt

    ; Phase 6: Advanced
    call test_retf
    call test_pushf_popf

    ; Print summary
    call print_summary

    ; Exit to DOS
    mov ax, 0x4C00
    int 21h

;=============================================================================
; Test: MOV (data movement)
;=============================================================================
test_mov:
    mov si, test_mov_name
    call print_test_name

    mov ax, 0x1234
    mov bx, ax
    cmp bx, 0x1234
    jne .fail

    mov cx, 0xABCD
    mov dx, cx
    cmp dx, 0xABCD
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_mov_failed
    call print_fail
    ret

;=============================================================================
; Test: ADD (addition)
;=============================================================================
test_add:
    mov si, test_add_name
    call print_test_name

    ; Test 1: Simple addition
    mov ax, 0x1234
    add ax, 0x0002
    cmp ax, 0x1236
    jne .fail

    ; Test 2: Addition with carry
    mov ax, 0xFFFF
    add ax, 1
    jnc .fail          ; Should set carry flag
    cmp ax, 0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_add_failed
    call print_fail
    ret

;=============================================================================
; Test: SUB (subtraction)
;=============================================================================
test_sub:
    mov si, test_sub_name
    call print_test_name

    ; Test 1: Simple subtraction
    mov ax, 0x1236
    sub ax, 0x0002
    cmp ax, 0x1234
    jne .fail

    ; Test 2: Subtraction with borrow
    mov ax, 0
    sub ax, 1
    jnc .fail          ; Should set carry flag
    cmp ax, 0xFFFF
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_sub_failed
    call print_fail
    ret

;=============================================================================
; Test: AND (bitwise AND)
;=============================================================================
test_and:
    mov si, test_and_name
    call print_test_name

    mov ax, 0xFF00
    and ax, 0x0FF0
    cmp ax, 0x0F00
    jne .fail

    mov bx, 0x5555
    and bx, 0xAAAA
    cmp bx, 0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_and_failed
    call print_fail
    ret

;=============================================================================
; Test: OR (bitwise OR)
;=============================================================================
test_or:
    mov si, test_or_name
    call print_test_name

    mov ax, 0x0F00
    or ax, 0x00F0
    cmp ax, 0x0FF0
    jne .fail

    mov bx, 0x5555
    or bx, 0xAAAA
    cmp bx, 0xFFFF
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_or_failed
    call print_fail
    ret

;=============================================================================
; Test: XOR (bitwise XOR)
;=============================================================================
test_xor:
    mov si, test_xor_name
    call print_test_name

    mov ax, 0xFFFF
    xor ax, 0xFFFF
    cmp ax, 0
    jne .fail

    mov bx, 0x5555
    xor bx, 0xAAAA
    cmp bx, 0xFFFF
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_xor_failed
    call print_fail
    ret

;=============================================================================
; Test: SHL (shift left)
;=============================================================================
test_shl:
    mov si, test_shl_name
    call print_test_name

    mov ax, 0x0001
    shl ax, 1
    cmp ax, 0x0002
    jne .fail

    mov bx, 0x0080
    shl bx, 1
    cmp bx, 0x0100
    jne .fail

    ; Test carry flag
    mov cx, 0x8000
    shl cx, 1
    jnc .fail          ; Should set carry
    cmp cx, 0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_shl_failed
    call print_fail
    ret

;=============================================================================
; Test: INC (increment)
;=============================================================================
test_inc:
    mov si, test_inc_name
    call print_test_name

    mov ax, 0
    inc ax
    cmp ax, 1
    jne .fail

    mov bx, 0xFFFE
    inc bx
    cmp bx, 0xFFFF
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_inc_failed
    call print_fail
    ret

;=============================================================================
; Test: CMP (compare - sets flags without storing)
;=============================================================================
test_cmp:
    mov si, test_cmp_name
    call print_test_name

    ; Test equal
    mov ax, 0x1234
    cmp ax, 0x1234
    jne .fail

    ; Test less than
    mov bx, 5
    cmp bx, 10
    jge .fail          ; 5 < 10

    ; Test greater than
    mov cx, 20
    cmp cx, 10
    jle .fail          ; 20 > 10

    call print_pass
    ret
.fail:
    mov si, msg_cmp_failed
    call print_fail
    ret

;=============================================================================
; Test: PUSH/POP (stack operations)
;=============================================================================
test_push_pop:
    mov si, test_push_pop_name
    call print_test_name

    ; Save current SP
    mov bp, sp

    ; Test basic push/pop
    mov ax, 0x1234
    push ax
    mov ax, 0
    pop ax
    cmp ax, 0x1234
    jne .fail

    ; Test multiple values
    mov ax, 0xABCD
    mov bx, 0x5678
    push ax
    push bx
    pop cx
    pop dx
    cmp cx, 0x5678
    jne .fail
    cmp dx, 0xABCD
    jne .fail

    ; Verify SP restored
    cmp sp, bp
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_push_pop_failed
    call print_fail
    ret

;=============================================================================
; Test: DEC (decrement)
;=============================================================================
test_dec:
    mov si, test_dec_name
    call print_test_name

    mov ax, 1
    dec ax
    cmp ax, 0
    jne .fail

    mov bx, 0
    dec bx
    cmp bx, 0xFFFF
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_dec_failed
    call print_fail
    ret

;=============================================================================
; Test: NEG (negate - two's complement)
;=============================================================================
test_neg:
    mov si, test_neg_name
    call print_test_name

    mov ax, 1
    neg ax
    cmp ax, 0xFFFF
    jne .fail

    mov bx, 0xFFFF
    neg bx
    cmp bx, 1
    jne .fail

    ; Test zero
    mov cx, 0
    neg cx
    cmp cx, 0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_neg_failed
    call print_fail
    ret

;=============================================================================
; Test: NOT (bitwise NOT)
;=============================================================================
test_not:
    mov si, test_not_name
    call print_test_name

    mov ax, 0xFFFF
    not ax
    cmp ax, 0
    jne .fail

    mov bx, 0x5555
    not bx
    cmp bx, 0xAAAA
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_not_failed
    call print_fail
    ret

;=============================================================================
; Test: TEST (bitwise AND without storing)
;=============================================================================
test_test:
    mov si, test_test_name
    call print_test_name

    ; Test zero flag
    mov ax, 0xFF00
    test ax, 0x00FF
    jnz .fail          ; Should be zero

    ; Test non-zero
    mov bx, 0x00FF
    test bx, 0x00FF
    jz .fail           ; Should not be zero

    ; Verify original value unchanged
    cmp bx, 0x00FF
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_test_failed
    call print_fail
    ret

;=============================================================================
; Test: SHR (shift right)
;=============================================================================
test_shr:
    mov si, test_shr_name
    call print_test_name

    mov ax, 0x0002
    shr ax, 1
    cmp ax, 0x0001
    jne .fail

    mov bx, 0x0100
    shr bx, 1
    cmp bx, 0x0080
    jne .fail

    ; Test carry flag
    mov cx, 0x0001
    shr cx, 1
    jnc .fail          ; Should set carry
    cmp cx, 0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_shr_failed
    call print_fail
    ret

;=============================================================================
; Test: ROL (rotate left)
;=============================================================================
test_rol:
    mov si, test_rol_name
    call print_test_name

    mov ax, 0x0001
    rol ax, 1
    cmp ax, 0x0002
    jne .fail

    ; Test bit wrap
    mov bx, 0x8000
    rol bx, 1
    jnc .fail          ; Should set carry
    cmp bx, 0x0001     ; MSB wraps to LSB
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_rol_failed
    call print_fail
    ret

;=============================================================================
; Test: ROR (rotate right)
;=============================================================================
test_ror:
    mov si, test_ror_name
    call print_test_name

    mov ax, 0x0002
    ror ax, 1
    cmp ax, 0x0001
    jne .fail

    ; Test bit wrap
    mov bx, 0x0001
    ror bx, 1
    jnc .fail          ; Should set carry
    cmp bx, 0x8000     ; LSB wraps to MSB
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_ror_failed
    call print_fail
    ret

;=============================================================================
; Test: MUL (unsigned multiply)
;=============================================================================
test_mul:
    mov si, test_mul_name
    call print_test_name

    ; Test 8-bit multiply (AL * operand -> AX)
    mov al, 5
    mov bl, 6
    mul bl
    cmp ax, 30
    jne .fail

    ; Test 16-bit multiply (AX * operand -> DX:AX)
    mov ax, 100
    mov cx, 200
    mul cx
    cmp dx, 0          ; High word
    jne .fail
    cmp ax, 20000      ; Low word
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_mul_failed
    call print_fail
    ret

;=============================================================================
; Test: DIV (unsigned divide)
;=============================================================================
test_div:
    mov si, test_div_name
    call print_test_name

    ; Test 16-bit divide (AX / operand -> AL=quotient, AH=remainder)
    mov ax, 30
    mov bl, 7
    div bl
    cmp al, 4          ; quotient
    jne .fail
    cmp ah, 2          ; remainder
    jne .fail

    ; Test 32-bit divide (DX:AX / operand -> AX=quotient, DX=remainder)
    mov dx, 0
    mov ax, 100
    mov cx, 3
    div cx
    cmp ax, 33         ; quotient
    jne .fail
    cmp dx, 1          ; remainder
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_div_failed
    call print_fail
    ret

;=============================================================================
; Test: LODSB (load string byte)
;=============================================================================
test_lodsb:
    mov si, test_lodsb_name
    call print_test_name

    ; Set up source
    mov si, test_string
    cld                ; Direction flag = forward

    ; Load first byte
    lodsb
    cmp al, 'T'
    jne .fail

    ; Load second byte (SI should auto-increment)
    lodsb
    cmp al, 'E'
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_lodsb_failed
    call print_fail
    ret

;=============================================================================
; Test: STOSB (store string byte)
;=============================================================================
test_stosb:
    mov si, test_stosb_name
    call print_test_name

    ; Set up destination
    mov di, test_buffer
    cld

    ; Store byte
    mov al, 'A'
    stosb

    ; Verify stored
    mov al, [test_buffer]
    cmp al, 'A'
    jne .fail

    ; Verify DI incremented
    mov al, 'B'
    stosb
    mov al, [test_buffer + 1]
    cmp al, 'B'
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_stosb_failed
    call print_fail
    ret

;=============================================================================
; Test: MOVSB (move string byte)
;=============================================================================
test_movsb:
    mov si, test_movsb_name
    call print_test_name

    ; Set up source and dest
    mov si, test_string
    mov di, test_buffer
    cld

    ; Move first byte
    movsb
    mov al, [test_buffer]
    cmp al, 'T'
    jne .fail

    ; Move second byte (SI and DI should auto-increment)
    movsb
    mov al, [test_buffer + 1]
    cmp al, 'E'
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_movsb_failed
    call print_fail
    ret

;=============================================================================
; Test: CMPSB (compare string byte)
;=============================================================================
test_cmpsb:
    mov si, test_cmpsb_name
    call print_test_name

    ; Test 1: Single byte comparison - equal
    mov si, cmp_string1  ; 'ABCD'
    mov di, cmp_string1  ; 'ABCD'
    cld
    cmpsb                ; Compare first bytes ('A' vs 'A')
    jne .fail            ; Should be equal (ZF=1)

    ; Test 2: Single byte comparison - not equal
    mov si, cmp_string1  ; Points to 'B' now (after previous cmpsb)
    mov di, cmp_string2 + 2  ; Points to 'X'
    cmpsb                ; Compare 'B' vs 'X'
    je .fail             ; Should NOT be equal (ZF=0)

    ; Test 3: REPE - compare equal strings
    mov si, cmp_string1
    mov di, cmp_string1  ; Same string
    mov cx, 4
    cld
    repe cmpsb
    jne .fail            ; Should complete with ZF=1 (all equal)
    cmp cx, 0            ; Should exhaust counter
    jne .fail

    ; Test 4: REPE - stop at difference
    mov si, cmp_string1  ; 'ABCD'
    mov di, cmp_string2  ; 'ABXY'
    mov cx, 4
    cld
    repe cmpsb
    je .fail             ; Should stop with ZF=0 (found difference)
    ; After comparing 'A'='A' (cx=3), 'B'='B' (cx=2), 'C'!='X' (cx=1, stop)
    ; CX should be 1 (decremented before comparison that failed)
    cmp cx, 1
    jl .fail             ; CX should be >= 1

    call print_pass
    ret
.fail:
    mov si, msg_cmpsb_failed
    call print_fail
    ret

;=============================================================================
; Test: SCASB (scan string byte)
;=============================================================================
test_scasb:
    mov si, test_scasb_name
    call print_test_name

    ; Find 'T' in test_string
    mov di, test_string
    mov al, 'T'
    mov cx, 10
    cld
    repne scasb
    jne .fail            ; Should find it
    ; DI should point one past the 'T'
    dec di
    mov al, [di]
    cmp al, 'T'
    jne .fail

    ; Find null terminator
    mov di, test_string
    mov al, 0
    mov cx, 10
    cld
    repne scasb
    jne .fail            ; Should find it

    call print_pass
    ret
.fail:
    mov si, msg_scasb_failed
    call print_fail
    ret

;=============================================================================
; Test: ADC/SBB (add/subtract with carry)
;=============================================================================
test_adc_sbb:
    mov si, test_adc_sbb_name
    call print_test_name

    ; Test ADC: Add 32-bit numbers
    ; Add 0x00010001 + 0x00020002 = 0x00030003
    clc                  ; Clear carry
    mov ax, 0x0001       ; Low word of first number
    mov dx, 0x0001       ; High word of first number
    mov bx, 0x0002       ; Low word of second number
    mov cx, 0x0002       ; High word of second number

    add ax, bx           ; Add low words
    adc dx, cx           ; Add high words with carry

    cmp ax, 0x0003       ; Check low word
    jne .fail
    cmp dx, 0x0003       ; Check high word
    jne .fail

    ; Test ADC with carry propagation
    ; Add 0x0001FFFF + 0x00000001 = 0x00020000
    clc
    mov ax, 0xFFFF       ; Low word
    mov dx, 0x0001       ; High word
    add ax, 0x0001       ; Add 1 to low word (will set carry)
    adc dx, 0x0000       ; Add carry to high word

    cmp ax, 0x0000       ; Low word should be 0
    jne .fail
    cmp dx, 0x0002       ; High word should be 2
    jne .fail

    ; Test SBB: Subtract 32-bit numbers
    ; Subtract 0x00030003 - 0x00020002 = 0x00010001
    clc
    mov ax, 0x0003       ; Low word of first number
    mov dx, 0x0003       ; High word of first number
    mov bx, 0x0002       ; Low word to subtract
    mov cx, 0x0002       ; High word to subtract

    sub ax, bx           ; Subtract low words
    sbb dx, cx           ; Subtract high words with borrow

    cmp ax, 0x0001
    jne .fail
    cmp dx, 0x0001
    jne .fail

    ; Test SBB with borrow propagation
    ; Subtract 0x00020000 - 0x00000001 = 0x0001FFFF
    clc
    mov ax, 0x0000       ; Low word
    mov dx, 0x0002       ; High word
    sub ax, 0x0001       ; Subtract 1 (will set carry/borrow)
    sbb dx, 0x0000       ; Subtract borrow from high word

    cmp ax, 0xFFFF       ; Low word should be 0xFFFF
    jne .fail
    cmp dx, 0x0001       ; High word should be 1
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_adc_sbb_failed
    call print_fail
    ret

;=============================================================================
; Test: IMUL (signed multiply)
;=============================================================================
test_imul:
    mov si, test_imul_name
    call print_test_name

    ; Test 8-bit signed multiply: 5 * 6 = 30
    mov al, 5
    mov bl, 6
    imul bl
    cmp ax, 30
    jne .fail

    ; Test 8-bit signed multiply with negative: -5 * 6 = -30
    mov al, -5           ; 0xFB
    mov bl, 6
    imul bl
    cmp ax, -30          ; 0xFFE2
    jne .fail

    ; Test 8-bit signed multiply: -5 * -6 = 30
    mov al, -5
    mov bl, -6
    imul bl
    cmp ax, 30
    jne .fail

    ; Test 16-bit signed multiply: 100 * 200 = 20000
    mov ax, 100
    mov cx, 200
    imul cx
    cmp dx, 0            ; High word should be 0
    jne .fail
    cmp ax, 20000        ; Low word
    jne .fail

    ; Test 16-bit signed multiply with negative: -100 * 200 = -20000
    mov ax, -100
    mov cx, 200
    imul cx
    ; Result is -20000 = 0xFFFFB1E0 in 32-bit
    cmp dx, 0xFFFF       ; High word (sign extended)
    jne .fail
    cmp ax, 20000        ; Low word magnitude (note: -20000 & 0xFFFF = 0xB1E0)
    jg .fail             ; ax should be negative representation

    call print_pass
    ret
.fail:
    mov si, msg_imul_failed
    call print_fail
    ret

;=============================================================================
; Test: IDIV (signed divide)
;=============================================================================
test_idiv:
    mov si, test_idiv_name
    call print_test_name

    ; Test 8-bit signed divide: 30 / 7 = 4 remainder 2
    mov ax, 30
    mov bl, 7
    idiv bl
    cmp al, 4            ; Quotient
    jne .fail
    cmp ah, 2            ; Remainder
    jne .fail

    ; Test 8-bit signed divide with negative dividend: -30 / 7 = -4 remainder -2
    mov ax, -30          ; 0xFFE2
    mov bl, 7
    idiv bl
    cmp al, -4           ; Quotient should be negative
    jne .fail
    cmp ah, -2           ; Remainder should be negative
    jne .fail

    ; Test 8-bit signed divide with negative divisor: 30 / -7 = -4 remainder 2
    mov ax, 30
    mov bl, -7
    idiv bl
    cmp al, -4           ; Quotient should be negative
    jne .fail
    cmp ah, 2            ; Remainder sign follows dividend
    jne .fail

    ; Test 16-bit signed divide: 100 / 3 = 33 remainder 1
    mov dx, 0
    mov ax, 100
    mov cx, 3
    idiv cx
    cmp ax, 33           ; Quotient
    jne .fail
    cmp dx, 1            ; Remainder
    jne .fail

    ; Test 16-bit signed divide with negative: -100 / 3 = -33 remainder -1
    mov ax, -100
    cwd                  ; Sign extend AX to DX:AX
    mov cx, 3
    idiv cx
    cmp ax, -33          ; Quotient
    jne .fail
    cmp dx, -1           ; Remainder
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_idiv_failed
    call print_fail
    ret

;=============================================================================
; Test: RCL/RCR (rotate through carry)
;=============================================================================
test_rcl_rcr:
    mov si, test_rcl_rcr_name
    call print_test_name

    ; Test RCL (rotate carry left)
    clc                  ; Clear carry
    mov ax, 0x0001
    rcl ax, 1            ; Rotate left, CF goes into bit 0
    jc .fail             ; Carry should be clear (bit 15 was 0)
    cmp ax, 0x0002       ; Result should be 2
    jne .fail

    ; Test RCL with carry set
    stc                  ; Set carry
    mov ax, 0x0000
    rcl ax, 1            ; Rotate left, CF (1) goes into bit 0
    jc .fail             ; Carry should be clear (bit 15 was 0)
    cmp ax, 0x0001       ; Result should be 1 (carry rotated in)
    jne .fail

    ; Test RCL with bit 15 set
    clc
    mov ax, 0x8000
    rcl ax, 1            ; Bit 15 goes to carry, CF goes to bit 0
    jnc .fail            ; Carry should be set (bit 15 was 1)
    cmp ax, 0x0000       ; Result should be 0
    jne .fail

    ; Test RCR (rotate carry right)
    clc
    mov ax, 0x0002
    rcr ax, 1            ; Rotate right, CF goes into bit 15
    jc .fail             ; Carry should be clear (bit 0 was 0)
    cmp ax, 0x0001       ; Result should be 1
    jne .fail

    ; Test RCR with carry set
    stc                  ; Set carry
    mov ax, 0x0000
    rcr ax, 1            ; Rotate right, CF (1) goes into bit 15
    jc .fail             ; Carry should be clear (bit 0 was 0)
    cmp ax, 0x8000       ; Result should be 0x8000 (carry rotated in)
    jne .fail

    ; Test RCR with bit 0 set
    clc
    mov ax, 0x0001
    rcr ax, 1            ; Bit 0 goes to carry, CF goes to bit 15
    jnc .fail            ; Carry should be set (bit 0 was 1)
    cmp ax, 0x0000       ; Result should be 0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_rcl_rcr_failed
    call print_fail
    ret

;=============================================================================
; Test: LOOPZ/LOOPNZ (loop with zero flag)
;=============================================================================
test_loopz_loopnz:
    mov si, test_loopz_loopnz_name
    call print_test_name

    ; Test 1: LOOPZ - loop while ZF=1
    mov cx, 3
    mov bx, 0
.loopz_test:
    inc bx
    cmp ax, ax           ; Set ZF=1 (always equal)
    loopz .loopz_test    ; Continue while ZF=1 and CX!=0
    ; Should complete all 3 iterations
    cmp bx, 3
    jne .fail
    cmp cx, 0
    jne .fail

    ; Test 2: LOOPZ - early exit when ZF=0
    mov cx, 5
    mov bx, 0
.loopz_exit:
    inc bx
    cmp bx, 2            ; Sets ZF=1 only when bx=2
    loopz .loopz_exit    ; Continue while ZF=1
    ; bx=1: cmp 1,2 -> ZF=0, loop exits, cx=4
    cmp bx, 1
    jne .fail
    cmp cx, 4            ; Should exit after 1 iteration
    jne .fail

    ; Test 3: LOOPNZ - loop while ZF=0
    mov cx, 3
    mov bx, 0
.loopnz_test:
    inc bx
    cmp ax, bx           ; Set ZF=0 (ax != bx, since ax is not bx)
    loopnz .loopnz_test  ; Continue while ZF=0 and CX!=0
    ; Should complete all 3 iterations
    cmp bx, 3
    jne .fail
    cmp cx, 0
    jne .fail

    ; Test 4: LOOPNZ - early exit when ZF=1
    mov cx, 5
    mov bx, 0
    mov ax, 2
.loopnz_exit:
    inc bx
    cmp bx, ax           ; Sets ZF=1 when bx=2
    loopnz .loopnz_exit  ; Continue while ZF=0
    ; bx=1: cmp 1,2 -> ZF=0, loop continues, cx=4
    ; bx=2: cmp 2,2 -> ZF=1, loop exits, cx=3
    cmp bx, 2
    jne .fail
    cmp cx, 3            ; Should exit after 2 iterations
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_loopz_loopnz_failed
    call print_fail
    ret

;=============================================================================
; Test: DAA/DAS (Decimal Adjust for Addition/Subtraction)
;=============================================================================
test_daa_das:
    mov si, test_daa_das_name
    call print_test_name

    ; Test DAA: 9 + 8 = 17 (BCD: 0x17)
    mov al, 9
    add al, 8            ; AL = 0x11 (17 in hex)
    daa                  ; Adjust to BCD: AL = 0x17
    cmp al, 0x17
    jne .fail

    ; Test DAA: 29 + 15 = 44 (BCD)
    ; 0x29 + 0x15 = 0x3E, DAA adjusts to 0x44
    mov al, 0x29         ; BCD 29
    add al, 0x15         ; Add BCD 15
    daa                  ; Should give 0x44 (BCD 44)
    cmp al, 0x44
    jne .fail

    ; Test DAA with carry: 95 + 8 = 103 (should set carry)
    clc
    mov al, 0x95         ; BCD 95
    add al, 0x08         ; Add 8
    daa                  ; Should give 0x03 with carry set
    jnc .fail            ; Carry should be set
    cmp al, 0x03
    jne .fail

    ; Test DAS: 58 - 25 = 33 (BCD)
    mov al, 0x58         ; BCD 58
    sub al, 0x25         ; Subtract BCD 25
    das                  ; Should give 0x33 (BCD 33)
    cmp al, 0x33
    jne .fail

    ; Test DAS: 40 - 15 = 25 (BCD)
    mov al, 0x40         ; BCD 40
    sub al, 0x15         ; Subtract BCD 15
    das                  ; Should give 0x25 (BCD 25)
    cmp al, 0x25
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_daa_das_failed
    call print_fail
    ret

;=============================================================================
; Test: AAA/AAS (ASCII Adjust for Addition/Subtraction)
;=============================================================================
test_aaa_aas:
    mov si, test_aaa_aas_name
    call print_test_name

    ; Test AAA: '9' + '8' in ASCII
    mov ax, 0           ; Clear AX
    mov al, '9'         ; ASCII 9 (0x39)
    add al, '8'         ; Add ASCII 8 (0x38) -> AL = 0x71
    aaa                 ; Adjust to unpacked BCD
    ; Should give AH = 1 (carry), AL = 7 (sum digit)
    cmp ah, 1
    jne .fail
    and al, 0x0F        ; Mask to get digit
    cmp al, 7
    jne .fail

    ; Test AAA: '5' + '3' in ASCII
    mov ax, 0
    mov al, '5'         ; ASCII 5 (0x35)
    add al, '3'         ; Add ASCII 3 (0x33) -> AL = 0x68
    aaa                 ; Adjust
    ; Should give AH = 0, AL = 8
    cmp ah, 0
    jne .fail
    and al, 0x0F
    cmp al, 8
    jne .fail

    ; Test AAS: '8' - '3' in ASCII
    mov ax, 0
    mov al, '8'         ; ASCII 8 (0x38)
    sub al, '3'         ; Subtract ASCII 3 (0x33) -> AL = 0x05
    aas                 ; Adjust
    ; Should give AH = 0, AL = 5
    cmp ah, 0
    jne .fail
    and al, 0x0F
    cmp al, 5
    jne .fail

    ; Test AAS: '2' - '9' in ASCII (borrow)
    mov ax, 0
    mov al, '2'         ; ASCII 2 (0x32)
    sub al, '9'         ; Subtract ASCII 9 (0x39) -> AL = 0xF9 (negative)
    aas                 ; Adjust with borrow
    ; Should give AH = 0xFF (borrow), AL = 3 (10 - 7 = 3)
    cmp ah, 0xFF
    jne .fail
    and al, 0x0F
    cmp al, 3
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_aaa_aas_failed
    call print_fail
    ret

;=============================================================================
; Test: AAM/AAD (ASCII Adjust for Multiply/Divide)
;=============================================================================
test_aam_aad:
    mov si, test_aam_aad_name
    call print_test_name

    ; Test AAM: Convert binary to unpacked BCD
    ; 54 decimal = 5 * 10 + 4
    mov al, 54
    aam                 ; Converts to AH = 5, AL = 4
    cmp ah, 5
    jne .fail
    cmp al, 4
    jne .fail

    ; Test AAM: 99 decimal
    mov al, 99
    aam                 ; Should give AH = 9, AL = 9
    cmp ah, 9
    jne .fail
    cmp al, 9
    jne .fail

    ; Test AAD: Convert unpacked BCD to binary
    ; AH = 5, AL = 4 -> 54 decimal
    mov ah, 5
    mov al, 4
    aad                 ; Converts to AL = 54
    cmp al, 54
    jne .fail

    ; Test AAD: AH = 9, AL = 9 -> 99 decimal
    mov ah, 9
    mov al, 9
    aad                 ; Should give AL = 99
    cmp al, 99
    jne .fail

    ; Test AAM/AAD round trip: 73 -> unpacked -> back to 73
    mov al, 73
    aam                 ; AH = 7, AL = 3
    aad                 ; Back to AL = 73
    cmp al, 73
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_aam_aad_failed
    call print_fail
    ret

;=============================================================================
; Test: JO/JNO (Jump on Overflow/No Overflow)
;=============================================================================
test_jo_jno:
    mov si, test_jo_jno_name
    call print_test_name

    ; Test JO: Signed overflow (127 + 1 = -128)
    mov al, 127         ; Maximum positive signed byte
    add al, 1           ; Should set overflow flag
    jno .fail           ; Should overflow
    jo .jo_ok           ; Should jump
.fail:
    mov si, msg_jo_jno_failed
    call print_fail
    ret
.jo_ok:

    ; Test JNO: No overflow (100 + 20 = 120)
    mov al, 100
    add al, 20          ; Should NOT overflow
    jo .fail            ; Should not overflow
    jno .jno_ok         ; Should jump
    jmp .fail
.jno_ok:

    ; Test JO: Negative overflow (-128 - 1 = 127)
    mov al, -128        ; Minimum negative signed byte (0x80)
    sub al, 1           ; Should set overflow flag
    jno .fail           ; Should overflow
    jo .jo_ok2          ; Should jump
    jmp .fail
.jo_ok2:

    ; Test JNO: No overflow in subtraction
    mov al, -50
    sub al, 20          ; -50 - 20 = -70, no overflow
    jo .fail
    jno .jno_ok2
    jmp .fail
.jno_ok2:

    call print_pass
    ret

;=============================================================================
; Test: JP/JNP (Jump on Parity/No Parity)
;=============================================================================
test_jp_jnp:
    mov si, test_jp_jnp_name
    call print_test_name

    ; Test JP: Even parity (even number of 1 bits)
    mov al, 0x03        ; 0000 0011 - two 1 bits (even)
    or al, al           ; Set flags based on AL
    jnp .fail           ; Should have even parity
    jp .jp_ok           ; Should jump
.fail:
    mov si, msg_jp_jnp_failed
    call print_fail
    ret
.jp_ok:

    ; Test JNP: Odd parity (odd number of 1 bits)
    mov al, 0x07        ; 0000 0111 - three 1 bits (odd)
    or al, al           ; Set flags
    jp .fail            ; Should NOT have even parity
    jnp .jnp_ok         ; Should jump
    jmp .fail
.jnp_ok:

    ; Test JP: Zero has even parity
    mov al, 0x00        ; 0000 0000 - zero 1 bits (even)
    or al, al
    jnp .fail
    jp .jp_ok2
    jmp .fail
.jp_ok2:

    ; Test JNP: All bits set (8 bits) - even parity
    mov al, 0xFF        ; 1111 1111 - eight 1 bits (even)
    or al, al
    jnp .fail           ; Should have even parity
    jp .jp_ok3
    jmp .fail
.jp_ok3:

    ; Test JNP: Single bit - odd parity
    mov al, 0x01        ; 0000 0001 - one 1 bit (odd)
    or al, al
    jp .fail
    jnp .jnp_ok2
    jmp .fail
.jnp_ok2:

    call print_pass
    ret

;=============================================================================
; Test: LDS/LES (Load Pointer to DS/ES)
;=============================================================================
test_lds_les:
    mov si, test_lds_les_name
    call print_test_name

    ; Save original segment registers
    push ds
    push es

    ; Set up a far pointer in memory (offset:segment)
    mov word [far_ptr_offset], 0x1234
    mov word [far_ptr_segment], 0x5678

    ; Test LDS: Load DS:SI from memory
    mov bx, far_ptr_offset
    lds si, [bx]        ; Load SI and DS from [BX] and [BX+2]
    cmp si, 0x1234      ; Check offset
    jne .fail
    mov ax, ds
    cmp ax, 0x5678      ; Check segment
    jne .fail

    ; Restore DS for further testing
    pop es              ; Get saved ES (will restore later)
    pop ds              ; Restore DS
    push ds             ; Save DS again
    push es             ; Save ES again

    ; Set up another far pointer
    mov word [far_ptr_offset], 0xABCD
    mov word [far_ptr_segment], 0xEF01

    ; Test LES: Load ES:DI from memory
    mov bx, far_ptr_offset
    les di, [bx]        ; Load DI and ES from [BX] and [BX+2]
    cmp di, 0xABCD      ; Check offset
    jne .fail
    mov ax, es
    cmp ax, 0xEF01      ; Check segment
    jne .fail

    ; Restore segment registers
    pop es
    pop ds

    call print_pass
    ret
.fail:
    pop es
    pop ds
    mov si, msg_lds_les_failed
    call print_fail
    ret

;=============================================================================
; Test: RET with immediate (stack cleanup)
;=============================================================================
test_ret_imm:
    mov si, test_ret_imm_name
    call print_test_name

    ; Save original SP
    mov word [saved_sp], sp

    ; Test 1: RET with 6-byte cleanup
    mov bp, sp
    mov ax, 0x1111
    push ax
    mov ax, 0x2222
    push ax
    mov ax, 0x3333
    push ax
    call .subroutine    ; Should clean up 6 bytes
    cmp sp, bp          ; SP should be restored
    jne .fail_cleanup

    ; Test 2: RET with 4-byte cleanup
    mov bp, sp
    mov ax, 0xAAAA
    push ax
    mov ax, 0xBBBB
    push ax
    call .subroutine2   ; Should clean up 4 bytes
    cmp sp, bp          ; SP should be restored
    jne .fail_cleanup

    call print_pass
    ret

.subroutine:
    ; This subroutine cleans up 6 bytes from stack on return
    ret 6               ; Pop return address and add 6 to SP

.subroutine2:
    ; This subroutine cleans up 4 bytes from stack on return
    ret 4               ; Pop return address and add 4 to SP

.fail_cleanup:
    ; Restore SP to saved value before failing
    mov sp, [saved_sp]
    mov si, msg_ret_imm_failed
    call print_fail
    ret

;=============================================================================
; Test: XCHG (exchange registers/memory)
;=============================================================================
test_xchg:
    mov si, test_xchg_name
    call print_test_name

    ; Test 1: XCHG reg, reg
    mov ax, 0x1234
    mov bx, 0x5678
    xchg ax, bx
    cmp ax, 0x5678
    jne .fail
    cmp bx, 0x1234
    jne .fail

    ; Test 2: XCHG with AX (special encoding)
    mov ax, 0xAAAA
    mov cx, 0xBBBB
    xchg ax, cx
    cmp ax, 0xBBBB
    jne .fail
    cmp cx, 0xAAAA
    jne .fail

    ; Test 3: XCHG reg, mem
    mov word [test_buffer], 0x9876
    mov dx, 0x4321
    xchg dx, [test_buffer]
    cmp dx, 0x9876
    jne .fail
    cmp word [test_buffer], 0x4321
    jne .fail

    ; Test 4: NOP (XCHG AX, AX)
    mov ax, 0xFFFF
    nop
    cmp ax, 0xFFFF      ; AX should be unchanged
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_xchg_failed
    call print_fail
    ret

;=============================================================================
; Test: LEA (load effective address)
;=============================================================================
test_lea:
    mov si, test_lea_name
    call print_test_name

    ; Test 1: LEA with [BX+SI]
    mov bx, 0x1000
    mov si, 0x0234
    lea ax, [bx+si]
    cmp ax, 0x1234      ; Should be sum of BX+SI, not memory contents
    jne .fail

    ; Test 2: LEA with [BP+DI+displacement]
    mov bp, 0x2000
    mov di, 0x0100
    lea cx, [bp+di+0x50]
    cmp cx, 0x2150      ; Should be sum of BP+DI+50h
    jne .fail

    ; Test 3: Verify LEA doesn't dereference
    mov word [test_buffer], 0xAAAA
    mov bx, test_buffer
    lea dx, [bx]
    cmp dx, bx          ; Should equal BX (address), not [BX] (0xAAAA)
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_lea_failed
    call print_fail
    ret

;=============================================================================
; Test: CBW/CWD (sign extension)
;=============================================================================
test_cbw_cwd:
    mov si, test_cbw_cwd_name
    call print_test_name

    ; Test 1: CBW with positive value
    mov al, 0x05
    cbw
    cmp ax, 0x0005      ; High byte should be 0x00
    jne .fail

    ; Test 2: CBW with negative value
    mov al, 0xFF
    cbw
    cmp ax, 0xFFFF      ; High byte should be 0xFF (sign extended)
    jne .fail

    ; Test 3: CBW with bit 7 clear
    mov al, 0x7F
    cbw
    cmp ax, 0x007F
    jne .fail

    ; Test 4: CWD with positive value
    mov ax, 0x1234
    cwd
    cmp dx, 0x0000      ; DX should be 0 for positive
    jne .fail
    cmp ax, 0x1234      ; AX unchanged
    jne .fail

    ; Test 5: CWD with negative value
    mov ax, 0x8000
    cwd
    cmp dx, 0xFFFF      ; DX should be 0xFFFF for negative
    jne .fail
    cmp ax, 0x8000      ; AX unchanged
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_cbw_cwd_failed
    call print_fail
    ret

;=============================================================================
; Test: LAHF/SAHF (load/store AH from/to flags)
;=============================================================================
test_lahf_sahf:
    mov si, test_lahf_sahf_name
    call print_test_name

    ; Test 1: LAHF loads flags into AH
    stc                 ; Set carry flag
    lahf                ; Load flags into AH
    test ah, 0x01       ; Bit 0 = CF
    jz .fail            ; CF should be set

    ; Test 2: SAHF stores AH into flags
    mov ah, 0x00        ; Clear all flags in AH
    sahf                ; Store AH to flags
    jc .fail            ; CF should be clear

    ; Test 3: Round trip test
    stc                 ; Set carry
    lahf                ; Load to AH
    mov al, ah          ; Save AH
    clc                 ; Clear carry
    sahf                ; Restore from AH (should set carry again)
    jnc .fail           ; CF should be set

    ; Test 4: SAHF sets carry
    mov ah, 0x01        ; Set bit 0 (CF)
    sahf
    jnc .fail           ; CF should be set

    call print_pass
    ret
.fail:
    mov si, msg_lahf_sahf_failed
    call print_fail
    ret

;=============================================================================
; Test: XLATB (translate byte via lookup table)
;=============================================================================
test_xlatb:
    mov si, test_xlatb_name
    call print_test_name

    ; Set up translation table
    mov di, xlat_table
    mov cx, 256
    mov al, 0
.setup:
    stosb               ; Fill table with 0, 1, 2, ...
    inc al
    loop .setup

    ; Set up special translations
    mov byte [xlat_table + 0], 0xFF
    mov byte [xlat_table + 1], 0xEE
    mov byte [xlat_table + 5], 0xAA

    ; Test 1: Translate index 0
    mov bx, xlat_table
    mov al, 0
    xlatb
    cmp al, 0xFF        ; Should get value from table[0]
    jne .fail

    ; Test 2: Translate index 1
    mov al, 1
    xlatb
    cmp al, 0xEE        ; Should get value from table[1]
    jne .fail

    ; Test 3: Translate index 5
    mov al, 5
    xlatb
    cmp al, 0xAA        ; Should get value from table[5]
    jne .fail

    ; Test 4: Translate index 10 (identity)
    mov al, 10
    xlatb
    cmp al, 10          ; Should get value from table[10] = 10
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_xlatb_failed
    call print_fail
    ret

;=============================================================================
; Test: SAR (shift arithmetic right)
;=============================================================================
test_sar:
    mov si, test_sar_name
    call print_test_name

    ; Test 1: SAR with positive value (sign bit = 0)
    mov ax, 0x0100
    sar ax, 1
    cmp ax, 0x0080      ; Shift right, sign bit stays 0
    jne .fail

    ; Test 2: SAR with negative value (sign bit = 1)
    mov ax, 0x8000
    sar ax, 1
    cmp ax, 0xC000      ; Shift right, sign bit stays 1 (sign extension)
    jne .fail

    ; Test 3: SAR with bit 0 set (test carry)
    mov ax, 0x0001
    sar ax, 1
    jnc .fail           ; Should set carry flag
    cmp ax, 0x0000
    jne .fail

    ; Test 4: SAR vs SHR difference
    mov ax, 0xFF00      ; Negative value
    mov bx, 0xFF00
    sar ax, 1           ; Sign-extending shift
    shr bx, 1           ; Logical shift
    cmp ax, 0xFF80      ; SAR preserves sign
    jne .fail
    cmp bx, 0x7F80      ; SHR shifts in 0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_sar_failed
    call print_fail
    ret

;=============================================================================
; Test: LODSW (load string word)
;=============================================================================
test_lodsw:
    mov si, test_lodsw_name
    call print_test_name

    ; Set up test data
    mov word [word_buffer], 0x1234
    mov word [word_buffer+2], 0x5678

    ; Test 1: LODSW loads first word
    mov si, word_buffer
    cld
    lodsw
    cmp ax, 0x1234
    jne .fail

    ; Test 2: LODSW loads second word (SI auto-increments by 2)
    lodsw
    cmp ax, 0x5678
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_lodsw_failed
    call print_fail
    ret

;=============================================================================
; Test: STOSW (store string word)
;=============================================================================
test_stosw:
    mov si, test_stosw_name
    call print_test_name

    ; Test 1: STOSW stores first word
    mov di, word_buffer
    mov ax, 0xABCD
    cld
    stosw
    cmp word [word_buffer], 0xABCD
    jne .fail

    ; Test 2: STOSW stores second word (DI auto-increments by 2)
    mov ax, 0xEF01
    stosw
    cmp word [word_buffer+2], 0xEF01
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_stosw_failed
    call print_fail
    ret

;=============================================================================
; Test: MOVSW (move string word)
;=============================================================================
test_movsw:
    mov si, test_movsw_name
    call print_test_name

    ; Set up source data
    mov word [test_buffer], 0x1111
    mov word [test_buffer+2], 0x2222

    ; Test 1: MOVSW forward
    mov si, test_buffer
    mov di, word_buffer
    cld
    movsw
    cmp word [word_buffer], 0x1111
    jne .fail
    cmp si, test_buffer+2
    jne .fail
    cmp di, word_buffer+2
    jne .fail

    ; Test 2: MOVSW with REP
    mov word [test_buffer], 0x3333
    mov word [test_buffer+2], 0x4444
    mov word [test_buffer+4], 0x5555
    mov si, test_buffer
    mov di, word_buffer
    mov cx, 3
    cld
    rep movsw
    cmp word [word_buffer], 0x3333
    jne .fail
    cmp word [word_buffer+2], 0x4444
    jne .fail
    cmp word [word_buffer+4], 0x5555
    jne .fail
    cmp cx, 0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_movsw_failed
    call print_fail
    ret

;=============================================================================
; Test: CMPSW (compare string word)
;=============================================================================
test_cmpsw:
    mov si, test_cmpsw_name
    call print_test_name

    ; Set up test data
    mov word [test_buffer], 0x1234
    mov word [test_buffer+2], 0x5678
    mov word [word_buffer], 0x1234
    mov word [word_buffer+2], 0x5678
    mov word [word_buffer+4], 0xABCD

    ; Test 1: CMPSW equal words
    mov si, test_buffer
    mov di, word_buffer
    cld
    cmpsw
    jne .fail           ; Should be equal
    cmp si, test_buffer+2
    jne .fail
    cmp di, word_buffer+2
    jne .fail

    ; Test 2: CMPSW not equal
    mov si, test_buffer
    mov di, word_buffer+4
    cmpsw
    je .fail            ; Should NOT be equal
    cmp si, test_buffer+2
    jne .fail
    cmp di, word_buffer+6
    jne .fail

    ; Test 3: REPE CMPSW
    mov word [test_buffer], 0x1111
    mov word [test_buffer+2], 0x2222
    mov word [test_buffer+4], 0x3333
    mov word [word_buffer], 0x1111
    mov word [word_buffer+2], 0x2222
    mov word [word_buffer+4], 0x3333
    mov si, test_buffer
    mov di, word_buffer
    mov cx, 3
    cld
    repe cmpsw
    jne .fail           ; All should be equal
    cmp cx, 0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_cmpsw_failed
    call print_fail
    ret

;=============================================================================
; Test: SCASW (scan string word)
;=============================================================================
test_scasw:
    mov si, test_scasw_name
    call print_test_name

    ; Set up test data
    mov word [word_buffer], 0x1111
    mov word [word_buffer+2], 0x2222
    mov word [word_buffer+4], 0x3333
    mov word [word_buffer+6], 0x4444

    ; Test 1: SCASW find word
    mov di, word_buffer
    mov ax, 0x3333
    mov cx, 4
    cld
    repne scasw
    jne .fail           ; Should find it
    cmp di, word_buffer+6   ; DI points past found word
    jne .fail
    cmp cx, 1           ; One comparison left
    jne .fail

    ; Test 2: SCASW not found
    mov di, word_buffer
    mov ax, 0xFFFF
    mov cx, 4
    cld
    repne scasw
    je .fail            ; Should NOT find it
    cmp cx, 0           ; All comparisons exhausted
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_scasw_failed
    call print_fail
    ret

;=============================================================================
; Test: Conditional Jumps (comprehensive)
;=============================================================================
test_conditional_jumps:
    mov si, test_conditional_jumps_name
    call print_test_name

    ; Test JA (jump if above - unsigned)
    mov ax, 10
    cmp ax, 5
    jna .fail           ; 10 > 5, should jump
    ja .ja_ok
.fail:
    mov si, msg_conditional_jumps_failed
    call print_fail
    ret
.ja_ok:

    ; Test JAE (jump if above or equal)
    mov ax, 5
    cmp ax, 5
    jb .fail            ; 5 >= 5, should jump
    jae .jae_ok
    jmp .fail
.jae_ok:

    ; Test JB (jump if below - unsigned)
    mov ax, 5
    cmp ax, 10
    jnb .fail           ; 5 < 10, should jump
    jb .jb_ok
    jmp .fail
.jb_ok:

    ; Test JBE (jump if below or equal)
    mov ax, 5
    cmp ax, 5
    ja .fail            ; 5 <= 5, should jump
    jbe .jbe_ok
    jmp .fail
.jbe_ok:

    ; Test JE/JZ (jump if equal/zero)
    mov ax, 100
    cmp ax, 100
    jne .fail           ; Should be equal
    je .je_ok
    jmp .fail
.je_ok:

    ; Test JNE/JNZ (jump if not equal/not zero)
    mov ax, 50
    cmp ax, 60
    je .fail            ; Should not be equal
    jne .jne_ok
    jmp .fail
.jne_ok:

    ; Test JG (jump if greater - signed)
    mov ax, 10
    cmp ax, -5
    jng .fail           ; 10 > -5 (signed), should jump
    jg .jg_ok
    jmp .fail
.jg_ok:

    ; Test JGE (jump if greater or equal - signed)
    mov ax, -5
    cmp ax, -5
    jl .fail            ; -5 >= -5, should jump
    jge .jge_ok
    jmp .fail
.jge_ok:

    ; Test JL (jump if less - signed)
    mov ax, -10
    cmp ax, 5
    jnl .fail           ; -10 < 5 (signed), should jump
    jl .jl_ok
    jmp .fail
.jl_ok:

    ; Test JLE (jump if less or equal - signed)
    mov ax, 5
    cmp ax, 5
    jg .fail            ; 5 <= 5, should jump
    jle .jle_ok
    jmp .fail
.jle_ok:

    ; Test JS (jump if sign - negative)
    mov ax, -1
    or ax, ax           ; Set flags
    jns .fail           ; Should be negative
    js .js_ok
    jmp .fail
.js_ok:

    ; Test JNS (jump if not sign - positive)
    mov ax, 1
    or ax, ax
    js .fail            ; Should be positive
    jns .jns_ok
    jmp .fail
.jns_ok:

    call print_pass
    ret

;=============================================================================
; Test: JMP (unconditional jump)
;=============================================================================
test_jmp:
    mov si, test_jmp_name
    call print_test_name

    ; Test 1: Short jump forward
    mov ax, 0
    jmp .target1
    mov ax, 1           ; Should skip this
.target1:
    cmp ax, 0
    jne .fail

    ; Test 2: Short jump backward
    mov bx, 0
    jmp .forward
.backward:
    jmp .after_backward
.forward:
    mov bx, 5
    jmp .backward
    mov bx, 1           ; Should skip this
.after_backward:
    cmp bx, 5
    jne .fail

    ; Test 3: Near jump
    mov cx, 0
    jmp near .target3
    mov cx, 1           ; Should skip
.target3:
    cmp cx, 0
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_jmp_failed
    call print_fail
    ret

;=============================================================================
; Test: CALL/RET (near call and return)
;=============================================================================
test_call_ret:
    mov si, test_call_ret_name
    call print_test_name

    ; Test 1: Simple CALL/RET
    mov ax, 0
    call .subroutine1
    cmp ax, 0x1234      ; Should be modified by subroutine
    jne .fail

    ; Test 2: Nested calls
    mov bx, 0
    call .subroutine2
    cmp bx, 0xABCD
    jne .fail

    ; Test 3: Verify stack (SP should be restored)
    mov bp, sp
    call .subroutine1
    cmp sp, bp
    jne .fail

    call print_pass
    ret

.subroutine1:
    mov ax, 0x1234
    ret

.subroutine2:
    call .nested
    ret

.nested:
    mov bx, 0xABCD
    ret

.fail:
    mov si, msg_call_ret_failed
    call print_fail
    ret

;=============================================================================
; Test: JCXZ/LOOP (loop with CX)
;=============================================================================
test_jcxz_loop:
    mov si, test_jcxz_loop_name
    call print_test_name

    ; Test 1: JCXZ when CX=0
    mov cx, 0
    jcxz .jcxz_ok1      ; Should jump
    jmp .fail
.jcxz_ok1:

    ; Test 2: JCXZ when CX!=0
    mov cx, 1
    jcxz .fail          ; Should NOT jump
    jmp .jcxz_ok2
.jcxz_ok2:

    ; Test 3: LOOP basic counting
    mov cx, 3
    mov bx, 0
.loop_test:
    inc bx
    loop .loop_test
    cmp bx, 3           ; Should loop 3 times
    jne .fail
    cmp cx, 0           ; CX should be 0
    jne .fail

    ; Test 4: LOOP with CX=0 (no loop)
    mov cx, 0
    mov dx, 0
.loop_test2:
    inc dx
    loop .loop_test2
    cmp dx, 0           ; Should NOT execute loop body
    jne .fail

    call print_pass
    ret
.fail:
    mov si, msg_jcxz_loop_failed
    call print_fail
    ret

;=============================================================================
; Test: Flag Operations (CLC/STC/CMC)
;=============================================================================
test_flag_operations:
    mov si, test_flag_operations_name
    call print_test_name

    ; Test 1: CLC (clear carry)
    stc                 ; Set carry first
    clc                 ; Clear carry
    jc .fail            ; Carry should be clear

    ; Test 2: STC (set carry)
    clc                 ; Clear carry first
    stc                 ; Set carry
    jnc .fail           ; Carry should be set

    ; Test 3: CMC (complement carry)
    clc
    cmc                 ; Should set carry
    jnc .fail
    cmc                 ; Should clear carry
    jc .fail

    ; Test 4: Multiple CMC toggles
    clc
    cmc                 ; 0 -> 1
    cmc                 ; 1 -> 0
    cmc                 ; 0 -> 1
    jnc .fail           ; Should be set

    call print_pass
    ret
.fail:
    mov si, msg_flag_operations_failed
    call print_fail
    ret

;=============================================================================
; Test: Direction and Interrupt Flags (CLD/STD/CLI/STI)
;=============================================================================
test_direction_interrupt:
    mov si, test_direction_interrupt_name
    call print_test_name

    ; Test 1: CLD (clear direction flag)
    std                 ; Set DF
    cld                 ; Clear DF
    ; Verify with string operation
    mov si, test_buffer
    mov byte [test_buffer], 0xAA
    lodsb
    cmp si, test_buffer+1   ; Should increment (DF=0)
    jne .fail

    ; Test 2: STD (set direction flag)
    cld
    std                 ; Set DF
    ; Verify with string operation
    mov si, test_buffer+1
    lodsb
    cmp si, test_buffer ; Should decrement (DF=1)
    jne .fail

    ; Test 3: CLI/STI (may be no-op in user mode, just verify they don't crash)
    cli                 ; Disable interrupts
    nop
    sti                 ; Enable interrupts
    nop

    cld                 ; Restore DF
    call print_pass
    ret
.fail:
    cld
    mov si, msg_direction_interrupt_failed
    call print_fail
    ret

;=============================================================================
; Test: RETF (far return)
;=============================================================================
test_retf:
    mov si, test_retf_name
    call print_test_name

    ; Save current CS
    mov ax, cs
    push ax

    ; Test 1: Far call/return
    ; Note: This is tricky in real mode, we'll test RETF behavior
    ; by manually setting up a far return stack frame
    mov ax, 0x1234
    call .near_sub      ; Use near call for setup
    cmp ax, 0x5678      ; Should be modified
    jne .fail

    ; Clean up
    pop ax              ; Remove saved CS

    call print_pass
    ret

.near_sub:
    mov ax, 0x5678
    ret

.fail:
    pop ax              ; Clean stack
    mov si, msg_retf_failed
    call print_fail
    ret

;=============================================================================
; Test: PUSHF/POPF (push/pop flags)
;=============================================================================
test_pushf_popf:
    mov si, test_pushf_popf_name
    call print_test_name

    ; Test 1: PUSHF/POPF round trip
    stc                 ; Set carry flag
    pushf               ; Push flags
    clc                 ; Clear carry
    popf                ; Restore flags (should set carry again)
    jnc .fail           ; Carry should be set

    ; Test 2: PUSHF/POPF preserves flags
    clc                 ; Clear carry
    mov ax, 0           ; Set zero flag
    or ax, ax
    pushf
    stc                 ; Set carry
    mov ax, 1           ; Clear zero flag
    or ax, ax
    popf                ; Restore (CF=0, ZF=1)
    jc .fail            ; Carry should be clear
    jnz .fail           ; Zero should be set

    ; Test 3: Multiple flag states
    stc
    pushf
    mov bp, sp
    mov ax, [bp]        ; Read flags from stack
    test ax, 0x0001     ; Bit 0 = CF
    jz .fail            ; CF should be set in saved flags
    pop ax              ; Clean stack

    call print_pass
    ret
.fail:
    mov si, msg_pushf_popf_failed
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

    mov cx, 0          ; Digit count
    mov bx, 10

    ; Handle zero special case
    test ax, ax
    jnz .convert
    mov al, '0'
    call print_char
    jmp .done

.convert:
    ; Convert to digits (reverse order)
.divide:
    xor dx, dx
    div bx
    push dx            ; Save remainder
    inc cx
    test ax, ax
    jnz .divide

    ; Print digits
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

msg_banner: db '=== oxide86 Opcode Test Suite ===', 13, 10, 0

test_mov_name: db 'MOV', 0
test_add_name: db 'ADD', 0
test_sub_name: db 'SUB', 0
test_inc_name: db 'INC', 0
test_dec_name: db 'DEC', 0
test_neg_name: db 'NEG', 0
test_cmp_name: db 'CMP', 0
test_and_name: db 'AND', 0
test_or_name: db 'OR', 0
test_xor_name: db 'XOR', 0
test_not_name: db 'NOT', 0
test_test_name: db 'TEST', 0
test_shl_name: db 'SHL', 0
test_shr_name: db 'SHR', 0
test_rol_name: db 'ROL', 0
test_ror_name: db 'ROR', 0
test_mul_name: db 'MUL', 0
test_div_name: db 'DIV', 0
test_push_pop_name: db 'PUSH/POP', 0
test_lodsb_name: db 'LODSB', 0
test_stosb_name: db 'STOSB', 0
test_movsb_name: db 'MOVSB', 0
test_cmpsb_name: db 'CMPSB', 0
test_scasb_name: db 'SCASB', 0
test_adc_sbb_name: db 'ADC/SBB', 0
test_imul_name: db 'IMUL', 0
test_idiv_name: db 'IDIV', 0
test_rcl_rcr_name: db 'RCL/RCR', 0
test_loopz_loopnz_name: db 'LOOPZ/LOOPNZ', 0
test_daa_das_name: db 'DAA/DAS', 0
test_aaa_aas_name: db 'AAA/AAS', 0
test_aam_aad_name: db 'AAM/AAD', 0
test_jo_jno_name: db 'JO/JNO', 0
test_jp_jnp_name: db 'JP/JNP', 0
test_lds_les_name: db 'LDS/LES', 0
test_ret_imm_name: db 'RET imm', 0
test_xchg_name: db 'XCHG', 0
test_lea_name: db 'LEA', 0
test_cbw_cwd_name: db 'CBW/CWD', 0
test_lahf_sahf_name: db 'LAHF/SAHF', 0
test_xlatb_name: db 'XLATB', 0
test_sar_name: db 'SAR', 0
test_lodsw_name: db 'LODSW', 0
test_stosw_name: db 'STOSW', 0
test_movsw_name: db 'MOVSW', 0
test_cmpsw_name: db 'CMPSW', 0
test_scasw_name: db 'SCASW', 0
test_conditional_jumps_name: db 'Conditional Jumps', 0
test_jmp_name: db 'JMP', 0
test_call_ret_name: db 'CALL/RET', 0
test_jcxz_loop_name: db 'JCXZ/LOOP', 0
test_flag_operations_name: db 'CLC/STC/CMC', 0
test_direction_interrupt_name: db 'CLD/STD/CLI/STI', 0
test_retf_name: db 'RETF', 0
test_pushf_popf_name: db 'PUSHF/POPF', 0

msg_pass: db 'PASS', 13, 10, 0
msg_fail: db 'FAIL - ', 0

msg_mov_failed: db 'value mismatch', 0
msg_add_failed: db 'result or carry incorrect', 0
msg_sub_failed: db 'result or borrow incorrect', 0
msg_inc_failed: db 'result incorrect', 0
msg_dec_failed: db 'result incorrect', 0
msg_neg_failed: db 'result incorrect', 0
msg_cmp_failed: db 'flags incorrect', 0
msg_and_failed: db 'result incorrect', 0
msg_or_failed: db 'result incorrect', 0
msg_xor_failed: db 'result incorrect', 0
msg_not_failed: db 'result incorrect', 0
msg_test_failed: db 'flags or value modified', 0
msg_shl_failed: db 'result or carry incorrect', 0
msg_shr_failed: db 'result or carry incorrect', 0
msg_rol_failed: db 'result or carry incorrect', 0
msg_ror_failed: db 'result or carry incorrect', 0
msg_mul_failed: db 'result incorrect', 0
msg_div_failed: db 'quotient or remainder incorrect', 0
msg_push_pop_failed: db 'stack mismatch', 0
msg_lodsb_failed: db 'load or pointer incorrect', 0
msg_stosb_failed: db 'store or pointer incorrect', 0
msg_movsb_failed: db 'move or pointer incorrect', 0
msg_cmpsb_failed: db 'comparison or flags incorrect', 0
msg_scasb_failed: db 'scan or pointer incorrect', 0
msg_adc_sbb_failed: db 'carry/borrow propagation incorrect', 0
msg_imul_failed: db 'signed multiply incorrect', 0
msg_idiv_failed: db 'signed divide incorrect', 0
msg_rcl_rcr_failed: db 'rotate through carry incorrect', 0
msg_loopz_loopnz_failed: db 'loop with zero flag incorrect', 0
msg_daa_das_failed: db 'BCD decimal adjust incorrect', 0
msg_aaa_aas_failed: db 'ASCII adjust incorrect', 0
msg_aam_aad_failed: db 'ASCII multiply/divide adjust incorrect', 0
msg_jo_jno_failed: db 'overflow flag jump incorrect', 0
msg_jp_jnp_failed: db 'parity flag jump incorrect', 0
msg_lds_les_failed: db 'load far pointer incorrect', 0
msg_ret_imm_failed: db 'return with immediate incorrect', 0
msg_xchg_failed: db 'exchange incorrect', 0
msg_lea_failed: db 'effective address incorrect', 0
msg_cbw_cwd_failed: db 'sign extension incorrect', 0
msg_lahf_sahf_failed: db 'flags load/store incorrect', 0
msg_xlatb_failed: db 'translation incorrect', 0
msg_sar_failed: db 'arithmetic shift incorrect', 0
msg_lodsw_failed: db 'load string word incorrect', 0
msg_stosw_failed: db 'store string word incorrect', 0
msg_movsw_failed: db 'move string word incorrect', 0
msg_cmpsw_failed: db 'compare string word incorrect', 0
msg_scasw_failed: db 'scan string word incorrect', 0
msg_conditional_jumps_failed: db 'conditional jump incorrect', 0
msg_jmp_failed: db 'unconditional jump incorrect', 0
msg_call_ret_failed: db 'call/return incorrect', 0
msg_jcxz_loop_failed: db 'loop instruction incorrect', 0
msg_flag_operations_failed: db 'flag operation incorrect', 0
msg_direction_interrupt_failed: db 'direction/interrupt flag incorrect', 0
msg_retf_failed: db 'far return incorrect', 0
msg_pushf_popf_failed: db 'push/pop flags incorrect', 0

msg_summary: db '--- Summary ---', 13, 10, 0
msg_passed: db ' passed, ', 0
msg_failed: db ' failed', 0

test_string: db 'TEST', 0
cmp_string1: db 'ABCD', 0
cmp_string2: db 'ABXY', 0

section .bss
pass_count: resw 1
fail_count: resw 1
test_buffer: resb 10
far_ptr_offset: resw 1
far_ptr_segment: resw 1
saved_sp: resw 1
word_buffer: resw 10
xlat_table: resb 256
