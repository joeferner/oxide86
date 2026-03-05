; INT 14h - Serial Port Services Test
; Targets: COM1 (Port 0)
; Functions: 00h (Init), 01h (Write), 02h (Read), 03h (Status)

[CPU 8086]
org 0x0100

start:
    ; --- Function 00h: Initialize Serial Port ---
    ; AH = 00h
    ; AL = Parameters (9600 baud, no parity, 1 stop bit, 8 data bits = 11100011b or 0xE3)
    ; DX = 0000h (COM1)
    mov ah, 0x00
    mov al, 0xE3      
    xor dx, dx          ; DX = 0 for COM1
    int 0x14

    ; --- Function 01h: Write Character ---
    ; AH = 01h
    ; AL = Character to send ('8')
    ; DX = 0000h (COM1)
    mov ah, 0x01
    mov al, '8'
    xor dx, dx
    int 0x14
    
    ; Check bit 7 of AH (1 = Error/Timeout)
    test ah, 0x80
    jnz fail

    ; --- Function 02h: Read Character ---
    ; AH = 02h
    ; DX = 0000h (COM1)
    ; Returns: AL = character, AH = status
    mov ah, 0x02
    xor dx, dx
    int 0x14

    ; Verify status (AH should have bits 7, 4, 3, 2, 1 as 0 for success)
    test ah, 0x80
    jnz fail

    ; --- Verify Character ---
    ; Check if received character is '6'
    cmp al, '6'
    jne fail

    ; --- Function 03h: Get Port Status ---
    ; Just to demonstrate usage/verification
    mov ah, 0x03
    xor dx, dx
    int 0x14
    ; AH contains line status, AL contains modem status

    ; Success Exit (ErrorLevel 0)
    mov ax, 0x4C00
    int 0x21

fail:
    ; Failure Exit (ErrorLevel 1)
    mov ax, 0x4C01
    int 0x21
