; File I/O Test Program
; Tests INT 21h file operations (create, write, close, open, read, seek)
;
; This program demonstrates:
; - Creating a new file
; - Writing data to a file
; - Closing a file
; - Opening an existing file
; - Reading data from a file
; - Seeking within a file
; - Displaying results to console

[bits 16]
[org 0x0100]
[cpu 8086]

start:
    ; Display initial message
    mov dx, msg_start
    mov ah, 0x09
    int 0x21

    ; ===== Step 1: Create a file =====
    mov dx, msg_creating
    mov ah, 0x09
    int 0x21

    mov dx, filename
    mov cx, 0x00            ; Normal file attributes
    mov ah, 0x3C            ; Create file
    int 0x21
    jnc .create_ok
    jmp create_error
.create_ok:

    mov [file_handle], ax   ; Save file handle

    mov dx, msg_ok
    mov ah, 0x09
    int 0x21

    ; ===== Step 2: Write data to file =====
    mov dx, msg_writing
    mov ah, 0x09
    int 0x21

    mov bx, [file_handle]
    mov dx, file_content
    mov cx, file_content_len
    mov ah, 0x40            ; Write to file
    int 0x21
    jnc .write_ok
    jmp write_error
.write_ok:

    ; Check if all bytes were written
    cmp ax, file_content_len
    je .write_complete
    jmp write_error
.write_complete:

    mov dx, msg_ok
    mov ah, 0x09
    int 0x21

    ; ===== Step 3: Close the file =====
    mov dx, msg_closing
    mov ah, 0x09
    int 0x21

    mov bx, [file_handle]
    mov ah, 0x3E            ; Close file
    int 0x21
    jnc .close_ok
    jmp close_error
.close_ok:

    mov dx, msg_ok
    mov ah, 0x09
    int 0x21

    ; ===== Step 4: Open the file for reading =====
    mov dx, msg_opening
    mov ah, 0x09
    int 0x21

    mov dx, filename
    mov al, 0x00            ; Read-only mode
    mov ah, 0x3D            ; Open file
    int 0x21
    jnc .open_ok
    jmp open_error
.open_ok:

    mov [file_handle], ax   ; Save file handle

    mov dx, msg_ok
    mov ah, 0x09
    int 0x21

    ; ===== Step 5: Read data from file =====
    mov dx, msg_reading
    mov ah, 0x09
    int 0x21

    mov bx, [file_handle]
    mov dx, read_buffer
    mov cx, 256             ; Read up to 256 bytes
    mov ah, 0x3F            ; Read from file
    int 0x21
    jnc .read_ok
    jmp read_error
.read_ok:

    mov [bytes_read], ax    ; Save number of bytes read

    mov dx, msg_ok
    mov ah, 0x09
    int 0x21

    ; ===== Step 6: Display the content read =====
    mov dx, msg_content
    mov ah, 0x09
    int 0x21

    ; Display the read content (byte by byte)
    mov si, read_buffer
    mov cx, [bytes_read]

display_loop:
    jcxz display_done

    lodsb                   ; Load byte from [SI] into AL
    mov dl, al
    mov ah, 0x02            ; Write character
    int 0x21

    dec cx
    jmp short display_loop

display_done:
    mov dx, msg_newline
    mov ah, 0x09
    int 0x21

    ; ===== Step 7: Test seeking =====
    mov dx, msg_seeking
    mov ah, 0x09
    int 0x21

    mov bx, [file_handle]
    mov al, 0x00            ; Seek from start
    mov cx, 0x00
    mov dx, 0x07            ; Seek to offset 7 (skip "Hello, ")
    mov ah, 0x42            ; Seek
    int 0x21
    jnc .seek_ok
    jmp seek_error
.seek_ok:

    mov dx, msg_ok
    mov ah, 0x09
    int 0x21

    ; Read again from new position
    mov dx, msg_reading_again
    mov ah, 0x09
    int 0x21

    mov bx, [file_handle]
    mov dx, read_buffer
    mov cx, 256
    mov ah, 0x3F            ; Read from file
    int 0x21
    jnc .read2_ok
    jmp read_error
.read2_ok:

    mov [bytes_read], ax

    mov dx, msg_ok
    mov ah, 0x09
    int 0x21

    ; Display content from new position
    mov dx, msg_content
    mov ah, 0x09
    int 0x21

    mov si, read_buffer
    mov cx, [bytes_read]

display_loop2:
    jcxz display_done2

    lodsb
    mov dl, al
    mov ah, 0x02
    int 0x21

    dec cx
    jmp short display_loop2

display_done2:
    mov dx, msg_newline
    mov ah, 0x09
    int 0x21

    ; ===== Step 8: Close the file again =====
    mov bx, [file_handle]
    mov ah, 0x3E            ; Close file
    int 0x21

    ; ===== All tests passed! =====
    mov dx, msg_success
    mov ah, 0x09
    int 0x21

    jmp short exit

; Error handlers
create_error:
    mov dx, msg_create_err
    jmp short print_error

write_error:
    mov dx, msg_write_err
    jmp short print_error

close_error:
    mov dx, msg_close_err
    jmp short print_error

open_error:
    mov dx, msg_open_err
    jmp short print_error

read_error:
    mov dx, msg_read_err
    jmp short print_error

seek_error:
    mov dx, msg_seek_err
    jmp short print_error

print_error:
    mov ah, 0x09
    int 0x21
    ; Fall through to exit

exit:
    mov ah, 0x4C            ; Exit program
    mov al, 0x00
    int 0x21

; Data section
filename:           db 'test.txt', 0
file_content:       db 'Hello, World! This is a test file created by emu86.', 0x0D, 0x0A
file_content_len:   equ $ - file_content

file_handle:        dw 0
bytes_read:         dw 0
read_buffer:        times 256 db 0

; Messages
msg_start:          db '=== File I/O Test Program ===', 0x0D, 0x0A, '$'
msg_creating:       db 'Creating file... $'
msg_writing:        db 'Writing data... $'
msg_closing:        db 'Closing file... $'
msg_opening:        db 'Opening file... $'
msg_reading:        db 'Reading data... $'
msg_seeking:        db 'Seeking to offset 7... $'
msg_reading_again:  db 'Reading from new position... $'
msg_content:        db 'Content: $'
msg_ok:             db 'OK', 0x0D, 0x0A, '$'
msg_newline:        db 0x0D, 0x0A, '$'
msg_success:        db 0x0D, 0x0A, 'All tests passed successfully!', 0x0D, 0x0A, '$'

msg_create_err:     db 'ERROR: Failed to create file', 0x0D, 0x0A, '$'
msg_write_err:      db 'ERROR: Failed to write to file', 0x0D, 0x0A, '$'
msg_close_err:      db 'ERROR: Failed to close file', 0x0D, 0x0A, '$'
msg_open_err:       db 'ERROR: Failed to open file', 0x0D, 0x0A, '$'
msg_read_err:       db 'ERROR: Failed to read from file', 0x0D, 0x0A, '$'
msg_seek_err:       db 'ERROR: Failed to seek in file', 0x0D, 0x0A, '$'
