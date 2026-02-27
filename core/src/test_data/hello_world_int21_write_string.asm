; Simple test program for program loading

[CPU 8086]
org 0x0100          ; .COM files start at CS:0100h

start:
    mov ah, 0x09    ; DOS function: write string
    mov dx, msg     ; DS:DX points to message
    int 0x21        ; Call DOS

    hlt

msg db 'Hello World!', 0x0D, 0x0A, '$'
