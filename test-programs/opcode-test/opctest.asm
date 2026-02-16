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

msg_summary: db '--- Summary ---', 13, 10, 0
msg_passed: db ' passed, ', 0
msg_failed: db ' failed', 0

test_string: db 'TEST', 0

section .bss
pass_count: resw 1
fail_count: resw 1
test_buffer: resb 10
