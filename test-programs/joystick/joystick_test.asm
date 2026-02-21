; joystick_test.asm - Tests IBM Game Control Adapter (port 0x201)
;
; Reads and displays axis poll counts and button states for both joysticks in
; real-time. Fires RC one-shots and counts how many reads each axis timer stays
; high — a proxy for joystick position.
;
; Build: nasm -f bin joystick_test.asm -o joystick_test.com
; Run:   cargo run -p oxide86-native-gui -- --joystick-a [--joystick-b] joystick_test.com
;
; Screen layout:
;   Row  0: Title
;   Row  1: Usage hint
;   Row  3: "Joystick A:"
;   Row  4:   X/Y axis counts
;   Row  5:   Button states
;   Row  7: "Joystick B:"
;   Row  8:   X/Y axis counts
;   Row  9:   Button states
;   Row 11: Raw port hex value

[CPU 8086]
org 0x100

JOY_PORT  equ 0x0201
MAX_COUNT equ 500       ; max poll iterations per fire (avoids blocking on missing B)

; ─── Entry point ─────────────────────────────────────────────────────────────
start:
    ; Set 80x25 text mode (clears screen)
    mov ax, 0x0003
    int 0x10

    ; Hide cursor
    mov ah, 0x01
    mov cx, 0x2000
    int 0x10

    ; Print static labels (one time)
    call print_static_labels

; ─── Main loop ───────────────────────────────────────────────────────────────
main_loop:
    ; Non-blocking keypress check — exit on any key
    mov ah, 0x01
    int 0x16
    jnz .exit

    ; ── Fire RC one-shots ──
    mov dx, JOY_PORT
    out dx, al          ; any write triggers all axis timers

    ; ── Reset per-axis poll counts ──
    xor ax, ax
    mov [count_ax], ax
    mov [count_ay], ax
    mov [count_bx], ax
    mov [count_by], ax

    ; ── Count reads while each axis timer bit is high ──
    mov cx, MAX_COUNT
    .poll_loop:
        in al, dx           ; read port 0x201 (DX still = JOY_PORT)

        test al, 0x01
        jz .no_ax
        inc word [count_ax]
    .no_ax:
        test al, 0x02
        jz .no_ay
        inc word [count_ay]
    .no_ay:
        test al, 0x04
        jz .no_bx
        inc word [count_bx]
    .no_bx:
        test al, 0x08
        jz .no_by
        inc word [count_by]
    .no_by:

        ; All four timer bits gone — done early
        test al, 0x0F
        jz .poll_done

        dec cx
        jnz .poll_loop

    .poll_done:

    ; ── Read button state (fresh read after timers expired) ──
    in al, dx
    mov [port_val], al

    ; ── Update dynamic display values ──
    call update_display

    jmp main_loop

.exit:
    ; Consume key from buffer
    mov ah, 0x00
    int 0x16

    ; Restore cursor
    mov ah, 0x01
    mov cx, 0x0607
    int 0x10

    ; Clear screen and exit
    mov ax, 0x0003
    int 0x10
    mov ah, 0x4C
    xor al, al
    int 0x21

; ─── Print static labels (called once at startup) ────────────────────────────
print_static_labels:
    mov dh, 0
    mov dl, 0
    call set_cursor
    mov si, str_title
    call print_str

    mov dh, 1
    mov dl, 0
    call set_cursor
    mov si, str_hint
    call print_str

    mov dh, 3
    mov dl, 0
    call set_cursor
    mov si, str_joy_a_hdr
    call print_str

    mov dh, 4
    mov dl, 0
    call set_cursor
    mov si, str_axes_label
    call print_str

    mov dh, 5
    mov dl, 0
    call set_cursor
    mov si, str_btns_label
    call print_str

    mov dh, 7
    mov dl, 0
    call set_cursor
    mov si, str_joy_b_hdr
    call print_str

    mov dh, 8
    mov dl, 0
    call set_cursor
    mov si, str_axes_label
    call print_str

    mov dh, 9
    mov dl, 0
    call set_cursor
    mov si, str_btns_label
    call print_str

    mov dh, 11
    mov dl, 0
    call set_cursor
    mov si, str_port_label
    call print_str

    ret

; ─── Update dynamic display values ───────────────────────────────────────────
update_display:
    ; --- Joystick A X count (row 4, col 6) ---
    mov dh, 4
    mov dl, 6
    call set_cursor
    mov ax, [count_ax]
    call print_decimal4

    ; --- Joystick A Y count (row 4, col 17) ---
    mov dh, 4
    mov dl, 17
    call set_cursor
    mov ax, [count_ay]
    call print_decimal4

    ; --- Joystick A Button 1 (row 5, col 9) ---
    mov dh, 5
    mov dl, 9
    call set_cursor
    mov al, [port_val]
    test al, 0x10           ; bit 4: 0=pressed, 1=released
    jnz .a1_rel
    mov si, str_pressed
    jmp .a1_print
.a1_rel:
    mov si, str_released
.a1_print:
    call print_str

    ; --- Joystick A Button 2 (row 5, col 27) ---
    mov dh, 5
    mov dl, 27
    call set_cursor
    mov al, [port_val]
    test al, 0x20           ; bit 5
    jnz .a2_rel
    mov si, str_pressed
    jmp .a2_print
.a2_rel:
    mov si, str_released
.a2_print:
    call print_str

    ; --- Joystick B X count (row 8, col 6) ---
    mov dh, 8
    mov dl, 6
    call set_cursor
    mov ax, [count_bx]
    call print_decimal4

    ; --- Joystick B Y count (row 8, col 17) ---
    mov dh, 8
    mov dl, 17
    call set_cursor
    mov ax, [count_by]
    call print_decimal4

    ; --- Joystick B Button 1 (row 9, col 9) ---
    mov dh, 9
    mov dl, 9
    call set_cursor
    mov al, [port_val]
    test al, 0x40           ; bit 6
    jnz .b1_rel
    mov si, str_pressed
    jmp .b1_print
.b1_rel:
    mov si, str_released
.b1_print:
    call print_str

    ; --- Joystick B Button 2 (row 9, col 27) ---
    mov dh, 9
    mov dl, 27
    call set_cursor
    mov al, [port_val]
    test al, 0x80           ; bit 7
    jnz .b2_rel
    mov si, str_pressed
    jmp .b2_print
.b2_rel:
    mov si, str_released
.b2_print:
    call print_str

    ; --- Raw port value hex (row 11, col 20) ---
    mov dh, 11
    mov dl, 20
    call set_cursor
    mov al, [port_val]
    call print_hex_byte

    ret

; ─── Subroutines ─────────────────────────────────────────────────────────────

; set_cursor: position cursor at row DH, col DL
set_cursor:
    mov ah, 0x02
    mov bh, 0x00
    int 0x10
    ret

; print_str: print null-terminated string at SI
print_str:
    mov ah, 0x0E
    mov bh, 0x00
.loop:
    lodsb
    test al, al
    jz .done
    int 0x10
    jmp .loop
.done:
    ret

; print_decimal4: print AX as exactly 4 zero-padded decimal digits
; Uses dec_buf. Destroys AX, BX, CX, DX, DI.
print_decimal4:
    mov di, dec_buf + 3     ; fill from rightmost digit
    mov cx, 4
.loop:
    mov bx, 10
    xor dx, dx
    div bx                  ; AX = AX/10, DX = remainder
    add dl, '0'
    mov [di], dl
    dec di
    loop .loop
    mov si, dec_buf
    call print_str
    ret

; print_hex_byte: print AL as two uppercase hex chars
; Destroys AX, BX.
print_hex_byte:
    mov bl, al
    ; High nibble
    mov cl, 4
    shr al, cl
    call print_hex_nibble
    ; Low nibble
    mov al, bl
    and al, 0x0F
    call print_hex_nibble
    ret

; print_hex_nibble: print low 4 bits of AL as one hex char
print_hex_nibble:
    cmp al, 10
    jl .digit
    add al, 'A' - 10
    jmp .print
.digit:
    add al, '0'
.print:
    mov ah, 0x0E
    mov bh, 0x00
    int 0x10
    ret

; ─── Data ────────────────────────────────────────────────────────────────────

str_title:
    db 'Joystick Axis & Button Test  (Port 0x201)', 0

str_hint:
    db 'Press any key to exit', 0

;           "    X: ????    Y: ????"
;            0       8      15     22
str_joy_a_hdr:
    db 'Joystick A:', 0
str_joy_b_hdr:
    db 'Joystick B:', 0

;           "   X: ????    Y: ????"
;             0  5  8      18 22
str_axes_label:
    db '   X: ????    Y: ????', 0

;           "   Btn1: ????????    Btn2: ????????"
;             0  5  9            23  27
str_btns_label:
    db '   Btn1: ????????    Btn2: ????????', 0

str_port_label:
    db '  Raw port 0x201: 0x', 0

str_pressed:
    db 'Pressed ', 0      ; 8 chars (padded to match "Released")

str_released:
    db 'Released', 0      ; 8 chars

; Per-axis poll counts (how many reads the axis timer bit was high)
count_ax:   dw 0
count_ay:   dw 0
count_bx:   dw 0
count_by:   dw 0

; Last raw port read (for button bits 4-7)
port_val:   db 0

; 4-digit decimal print buffer (no null terminator in buffer; print_str adds it)
dec_buf:    db '0000', 0
