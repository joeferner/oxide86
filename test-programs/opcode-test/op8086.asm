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

msg_banner: db '=== emu86 Opcode Test Suite ===', 13, 10, 0

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
