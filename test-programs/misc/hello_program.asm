; Simple test program for program loading

[CPU 8086]
org 0x0100          ; .COM files start at CS:0100h

    mov ah, 0x09    ; DOS function: write string
    mov dx, msg     ; DS:DX points to message
    int 0x21        ; Call DOS

    mov ah, 0x4C    ; DOS function: exit
    mov al, 0       ; Return code 0
    int 0x21        ; Call DOS

msg db 'Hello from loaded program!', 0x0D, 0x0A, '$'
