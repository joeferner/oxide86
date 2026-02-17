; Test INT 16h AH=00h blocking behavior
; This program waits for keypresses and echoes them back
; Press ESC to exit

[CPU 8086]
org 0x100

start:
    ; Print initial prompt
    mov si, msg_prompt
    call print_string

main_loop:
    ; Wait for a keypress (blocking call)
    mov ah, 0x00        ; Function 00h: Read character (blocking)
    int 0x16            ; BIOS keyboard service

    ; AH = scan code, AL = ASCII code
    push ax             ; Save the key

    ; Check if ESC was pressed (scan code 0x01)
    cmp ah, 0x01
    je exit_program

    ; Print "Key pressed: "
    mov si, msg_key
    call print_string

    ; Print the ASCII character
    pop ax              ; Restore the key
    push ax
    mov dl, al          ; Character to print
    cmp dl, 0x20        ; Is it printable?
    jae .printable
    mov dl, '.'         ; Non-printable, use '.'
.printable:
    mov ah, 0x02        ; DOS print character
    int 0x21

    ; Print newline
    mov si, msg_newline
    call print_string

    pop ax              ; Clean up stack
    jmp main_loop

exit_program:
    ; Print exit message
    mov si, msg_exit
    call print_string

    ; Exit to DOS
    mov ax, 0x4C00
    int 0x21

; Print null-terminated string at DS:SI
print_string:
    push ax
    push si
.loop:
    lodsb               ; Load byte from DS:SI to AL, increment SI
    test al, al         ; Check if null terminator
    jz .done
    mov dl, al
    mov ah, 0x02        ; DOS print character
    int 0x21
    jmp .loop
.done:
    pop si
    pop ax
    ret

msg_prompt:     db 'INT 16h Blocking Test - Press keys (ESC to exit)', 13, 10, 0
msg_key:        db 'Key pressed: ', 0
msg_newline:    db 13, 10, 0
msg_exit:       db 'ESC pressed - exiting', 13, 10, 0
