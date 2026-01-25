; Test I/O port instructions
; Demonstrates all 8 I/O instructions (IN/OUT with immediate and DX addressing)

org 0x0100

start:
    ; Test 1: OUT with immediate port, IN with immediate port (byte)
    mov al, 0x42            ; Load test value into AL
    out 0x60, al            ; Write to port 0x60
    mov al, 0x00            ; Clear AL
    in al, 0x60             ; Read back from port 0x60
    ; AL should now be 0x42

    ; Test 2: OUT with DX register, IN with DX register (byte)
    mov dx, 0x3F8           ; COM1 serial port address
    mov al, 0x41            ; Load 'A' (0x41)
    out dx, al              ; Write to port in DX
    mov al, 0x00            ; Clear AL
    in al, dx               ; Read back from port in DX
    ; AL should now be 0x41

    ; Test 3: OUT with immediate port (word)
    mov ax, 0x1234          ; Load test word
    out 0x62, ax            ; Write word to port 0x62 (writes 0x34 to 0x62, 0x12 to 0x63)
    mov ax, 0x0000          ; Clear AX
    in ax, 0x62             ; Read word back from port 0x62
    ; AX should now be 0x1234

    ; Test 4: OUT with DX register (word)
    mov dx, 0x70            ; CMOS port address
    mov ax, 0xABCD          ; Load test word
    out dx, ax              ; Write word to port in DX
    mov ax, 0x0000          ; Clear AX
    in ax, dx               ; Read word back
    ; AX should now be 0xABCD

    ; Test 5: System control port test (port 0x61 echoes last write)
    mov al, 0x55            ; Test pattern
    out 0x61, al            ; Write to port 0x61
    mov al, 0x00            ; Clear AL
    in al, 0x61             ; Read back
    ; AL should be 0x55

    ; Exit program
    mov ah, 0x4C            ; DOS exit function
    int 0x21                ; Call DOS
