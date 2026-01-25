; Hello World using INT 21h (DOS-like services)
; This program demonstrates the use of BIOS interrupts

BITS 16
ORG 0x0100              ; COM file format - start at 0x0100

start:
    ; Set up data segment
    mov ax, cs
    mov ds, ax

    ; Print "Hello, World!" using INT 21h, AH=09h
    mov dx, message     ; DS:DX points to the string
    mov ah, 0x09        ; Function 09h - Write string to STDOUT
    int 0x21            ; Call DOS interrupt

    ; Exit program using INT 21h, AH=4Ch
    mov ah, 0x4C        ; Function 4Ch - Exit program
    mov al, 0           ; Return code 0
    int 0x21            ; Call DOS interrupt

message:
    db 'Hello, World!$' ; '$' is the string terminator for INT 21h/09h
