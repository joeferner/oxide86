; 8086 String Instructions Demo
; Demonstrates MOVS, CMPS, SCAS, LODS, STOS instructions
; These instructions operate on memory blocks using SI, DI, and the Direction Flag (DF)

[BITS 16]
[ORG 0x0000]

start:
    ; Initialize segment registers
    mov ax, 0x1000
    mov ds, ax          ; DS = 0x1000 (source segment)
    mov ax, 0x2000
    mov es, ax          ; ES = 0x2000 (destination segment)

    ;===========================================
    ; Test 1: STOSB - Store String Byte
    ; Fill memory with a pattern
    ;===========================================
    mov di, 0x0000      ; Start at ES:0000
    mov al, 0x41        ; 'A' character
    mov cx, 5           ; Store 5 bytes
    cld                 ; Clear direction flag (forward)
fill_loop:
    stosb               ; Store AL at ES:DI, increment DI
    loop fill_loop      ; Repeat CX times
    ; Memory at ES:0000-0004 now contains: 41 41 41 41 41

    ;===========================================
    ; Test 2: MOVSB - Move String Byte
    ; Copy data from DS to ES
    ;===========================================
    ; Set up source data at DS:0100
    mov si, 0x0100
    mov byte [ds:si], 0x48      ; 'H'
    mov byte [ds:si+1], 0x45    ; 'E'
    mov byte [ds:si+2], 0x4C    ; 'L'
    mov byte [ds:si+3], 0x4C    ; 'L'
    mov byte [ds:si+4], 0x4F    ; 'O'

    ; Copy to destination
    mov si, 0x0100      ; Source at DS:0100
    mov di, 0x0100      ; Destination at ES:0100
    mov cx, 5           ; Copy 5 bytes
    cld                 ; Forward direction
copy_loop:
    movsb               ; Copy byte from DS:SI to ES:DI, increment both
    loop copy_loop
    ; Memory at ES:0100-0104 now contains: 48 45 4C 4C 4F ("HELLO")

    ;===========================================
    ; Test 3: MOVSW - Move String Word
    ; Copy 16-bit words
    ;===========================================
    mov si, 0x0200
    mov word [ds:si], 0x1234
    mov word [ds:si+2], 0x5678

    mov si, 0x0200      ; Source
    mov di, 0x0200      ; Destination
    mov cx, 2           ; Copy 2 words
    cld
copy_word_loop:
    movsw               ; Copy word from DS:SI to ES:DI
    loop copy_word_loop
    ; Memory at ES:0200-0203 now contains: 34 12 78 56

    ;===========================================
    ; Test 4: LODSB - Load String Byte
    ; Load bytes from memory into AL
    ;===========================================
    mov si, 0x0100      ; Point to "HELLO" at DS:0100
    cld
    lodsb               ; AL = 'H' (0x48), SI increments
    lodsb               ; AL = 'E' (0x45), SI increments
    lodsb               ; AL = 'L' (0x4C), SI increments
    ; AL now contains 0x4C ('L')

    ;===========================================
    ; Test 5: SCASB - Scan String Byte
    ; Search for a byte in memory
    ;===========================================
    mov di, 0x0100      ; Point to "HELLO" at ES:0100
    mov al, 0x4C        ; Search for 'L'
    mov cx, 5           ; Search up to 5 bytes
    cld
scan_loop:
    scasb               ; Compare AL with ES:DI, increment DI
    je found_it         ; Jump if equal (ZF=1)
    loop scan_loop
    jmp not_found

found_it:
    ; Found 'L' at position
    mov bx, 0xF0F0      ; Mark success
    jmp continue

not_found:
    mov bx, 0x0000      ; Mark failure

continue:

    ;===========================================
    ; Test 6: CMPSB - Compare String Byte
    ; Compare two memory blocks
    ;===========================================
    ; Set up two strings at DS:0300 and ES:0300
    mov si, 0x0300
    mov byte [ds:si], 0x41      ; 'A'
    mov byte [ds:si+1], 0x42    ; 'B'
    mov byte [ds:si+2], 0x43    ; 'C'

    mov di, 0x0300
    mov byte [es:di], 0x41      ; 'A'
    mov byte [es:di+1], 0x42    ; 'B'
    mov byte [es:di+2], 0x43    ; 'C'

    ; Compare the strings
    mov si, 0x0300
    mov di, 0x0300
    mov cx, 3
    cld
cmp_loop:
    cmpsb               ; Compare DS:SI with ES:DI
    jne strings_differ  ; Jump if not equal (ZF=0)
    loop cmp_loop

    ; Strings match
    mov dx, 0xAAAA      ; Mark strings equal
    jmp after_cmp

strings_differ:
    mov dx, 0x0000      ; Mark strings different

after_cmp:

    ;===========================================
    ; Test 7: Direction Flag (DF) Test
    ; Demonstrate backward string operations with STD
    ;===========================================
    ; Fill memory backward
    mov di, 0x0404      ; Start at end of 5-byte block
    mov al, 0x5A        ; 'Z'
    mov cx, 5
    std                 ; Set direction flag (backward)
backward_fill:
    stosb               ; Store and decrement DI
    loop backward_fill
    ; Memory at ES:0400-0404 now contains: 5A 5A 5A 5A 5A

    cld                 ; Clear DF back to forward

    ;===========================================
    ; All tests complete
    ;===========================================
    hlt                 ; Halt execution
