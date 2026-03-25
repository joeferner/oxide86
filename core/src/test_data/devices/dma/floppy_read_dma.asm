; =============================================================================
; DMA Floppy Read Test
; CPU: 286
;
; Programs DMA channel 2 and the FDC directly (bypassing BIOS int 13h) to
; perform a DMA-mode READ DATA transfer from drive A: cylinder 0, head 0,
; sector 1 into disk_buf.
;
; The disk image is zeroed by the test fixture, so after a successful DMA
; transfer disk_buf must contain all 0x00 bytes.  The buffer is pre-filled
; with 0xCC so a missing or failed transfer produces a data mismatch.
;
; Note: FDC ports (0x3F2/0x3F4/0x3F5) are > 255 and must be accessed via DX.
;       DMA ports and page register (0x00–0x0F, 0x81) fit in 8 bits and use
;       the direct-port form.
;
; DMA channel 2 port map (DMA1):
;   0x04  channel 2 base/current address (two bytes via flip-flop)
;   0x05  channel 2 base/current count   (two bytes via flip-flop)
;   0x0A  single-channel mask
;   0x0B  mode register
;   0x0C  clear byte-pointer flip-flop
;   0x81  page register for channel 2
;
; DMA mode byte 0x46 (WRITE, single, no-auto-init, channel 2):
;   bits 7-6: 01 = single mode
;   bit  5:   0  = address increment
;   bit  4:   0  = no auto-init
;   bits 3-2: 01 = write (device → memory)
;   bits 1-0: 10 = channel 2
;
; FDC READ DATA (0x06) without the NDM bit selects DMA mode.
;
; Exit codes:
;   0x00  all tests passed
;   0x02  FDC command-phase timeout (FDC never became ready for a byte)
;   0x03  FDC result-phase timeout  (DMA transfer never completed)
;   0x04  FDC reported an error     (ST0 bits 7:6 != 0)
;   0x05  Data verify failed        (buffer byte != 0x00 after transfer)
; =============================================================================

[CPU 286]
org 0x0100

; FDC port constants (loaded into DX before each access)
FDC_DOR equ 0x3F2
FDC_MSR equ 0x3F4
FDC_DATA equ 0x3F5

start:
    ; -------------------------------------------------------------------------
    ; Pre-fill disk_buf with 0xCC so a missing DMA transfer is detectable.
    ; -------------------------------------------------------------------------
    cld
    mov  di, disk_buf
    mov  cx, 512
    mov  al, 0xCC
    rep  stosb

    ; -------------------------------------------------------------------------
    ; Program DMA channel 2: WRITE (device → memory), single, no auto-init.
    ; -------------------------------------------------------------------------

    ; Mask channel 2 before re-programming
    mov  al, 0x06          ; ch=2, set-mask bit (bit 2 = 1)
    out  0x0A, al

    ; Clear byte-pointer flip-flop
    xor  al, al
    out  0x0C, al

    ; Channel 2 base/current address = low 16 bits of physical address of disk_buf.
    ; Physical = DS*16 + offset.  Low 16 bits = (DS<<4 & 0xFFFF) + offset.
    mov  ax, ds
    shl  ax, 4             ; AX = (DS << 4) & 0xFFFF
    add  ax, disk_buf      ; AX = low 16 bits of physical address
    out  0x04, al          ; low byte  (flip-flop: false → true)
    xchg al, ah
    out  0x04, al          ; high byte (flip-flop: true → false)

    ; Page register for channel 2 (port 0x81) = bits 19:16 of physical address
    ; = DS >> 12
    mov  ax, ds
    shr  ax, 12
    out  0x81, al          ; 0x81 fits in 8 bits — direct port OK

    ; Clear flip-flop before count writes (any write to 0x0C clears it)
    out  0x0C, al

    ; Channel 2 base/current count = 511 (= 512 bytes − 1)
    mov  ax, 511
    out  0x05, al          ; low byte  = 0xFF
    xchg al, ah
    out  0x05, al          ; high byte = 0x01

    ; Mode: single | increment | no-auto-init | write (device→mem) | ch2
    mov  al, 0x46
    out  0x0B, al

    ; Unmask channel 2
    mov  al, 0x02          ; ch=2, clear-mask bit (bit 2 = 0)
    out  0x0A, al

    ; -------------------------------------------------------------------------
    ; Activate FDC: motor A on, DMA/IRQ enabled, nRESET high, drive 0.
    ; DOR 0x1C = 0001_1100: MotA=1, DMA=1, nRESET=1, drive=0
    ; -------------------------------------------------------------------------
    mov  dx, FDC_DOR
    mov  al, 0x1C
    out  dx, al

    ; -------------------------------------------------------------------------
    ; Send READ DATA command (0x06, DMA mode — no NDM bit) + 8 parameters.
    ; send_fdc_byte polls MSR for RQM=1/DIO=0 before writing each byte.
    ; -------------------------------------------------------------------------
    mov  al, 0x06          ; READ DATA, DMA mode
    call send_fdc_byte
    jc   fail_cmd_timeout

    mov  al, 0x00          ; drive_head: drive 0, head 0
    call send_fdc_byte
    jc   fail_cmd_timeout

    mov  al, 0x00          ; cylinder 0
    call send_fdc_byte
    jc   fail_cmd_timeout

    mov  al, 0x00          ; head 0
    call send_fdc_byte
    jc   fail_cmd_timeout

    mov  al, 0x01          ; sector 1
    call send_fdc_byte
    jc   fail_cmd_timeout

    mov  al, 0x02          ; N = 2 (512 bytes/sector)
    call send_fdc_byte
    jc   fail_cmd_timeout

    mov  al, 0x01          ; EOT = 1 (last sector on track)
    call send_fdc_byte
    jc   fail_cmd_timeout

    mov  al, 0x1B          ; GPL (gap length, 1.44 MB standard)
    call send_fdc_byte
    jc   fail_cmd_timeout

    mov  al, 0xFF          ; DTL (use N)
    call send_fdc_byte
    jc   fail_cmd_timeout

    ; -------------------------------------------------------------------------
    ; Wait for DMA transfer to complete: poll MSR for result phase.
    ;
    ; MSR bits 7,6,5 (RQM, DIO, NDM):
    ;   Result phase:   110 → MSR & 0xE0 == 0xC0
    ;   PIO exec phase: 111 → MSR & 0xE0 == 0xE0  (not result)
    ;   DMA exec phase: 000 → MSR & 0xE0 == 0x00  (not result)
    ;
    ; CX=0 gives 65536 iterations (~1.5M CPU cycles), enough for 512 DMA bytes.
    ; -------------------------------------------------------------------------
    mov  dx, FDC_MSR
    mov  cx, 0
.wait_result:
    in   al, dx
    and  al, 0xE0
    cmp  al, 0xC0          ; RQM=1, DIO=1, NDM=0 → result phase?
    je   .result_ready
    loop .wait_result

    jmp  fail_result_timeout

.result_ready:
    ; -------------------------------------------------------------------------
    ; Read 7 result bytes; check ST0.
    ; -------------------------------------------------------------------------
    call read_fdc_byte     ; ST0
    jc   fail_result_timeout
    mov  bl, al            ; save ST0

    call read_fdc_byte     ; ST1
    jc   fail_result_timeout
    call read_fdc_byte     ; ST2
    jc   fail_result_timeout
    call read_fdc_byte     ; C
    jc   fail_result_timeout
    call read_fdc_byte     ; H
    jc   fail_result_timeout
    call read_fdc_byte     ; R
    jc   fail_result_timeout
    call read_fdc_byte     ; N
    jc   fail_result_timeout

    ; ST0 bits 7:6 must be 00 (normal termination)
    mov  al, bl
    and  al, 0xC0
    jnz  fail_st0_error

    ; -------------------------------------------------------------------------
    ; Verify disk_buf: all bytes must be 0x00 (zeroed disk, DMA wrote them).
    ; -------------------------------------------------------------------------
    mov  si, disk_buf
    mov  cx, 512
.verify_loop:
    lodsb
    cmp  al, 0x00
    jne  fail_data
    loop .verify_loop

all_pass:
    xor  al, al
    jmp  exit_with_code

fail_cmd_timeout:
    mov  al, 0x02
    jmp  exit_with_code

fail_result_timeout:
    mov  al, 0x03
    jmp  exit_with_code

fail_st0_error:
    mov  al, 0x04
    jmp  exit_with_code

fail_data:
    mov  al, 0x05

exit_with_code:
    mov  ah, 0x4C
    int  0x21

; =============================================================================
; Subroutine: wait_rqm_ready
; Polls FDC MSR until RQM=1 and DIO=0 (ready to accept a command byte).
; Destroys: AX, CX, DX
; Returns: carry clear = success, carry set = timeout
; =============================================================================
wait_rqm_ready:
    mov  dx, FDC_MSR
    mov  cx, 0             ; 65536 iterations
.loop:
    in   al, dx
    test al, 0x80          ; RQM set?
    jz   .not_ready
    test al, 0x40          ; DIO clear (FDC ready to receive)?
    jz   .ready
.not_ready:
    loop .loop
    stc
    ret
.ready:
    clc
    ret

; =============================================================================
; Subroutine: send_fdc_byte
; Waits for FDC ready then writes AL to FDC data port (0x3F5).
; Destroys: CX, DX; preserves AX
; Returns: carry clear = success, carry set = timeout
; =============================================================================
send_fdc_byte:
    push ax
    call wait_rqm_ready
    jc   .timeout
    pop  ax
    mov  dx, FDC_DATA
    out  dx, al
    clc
    ret
.timeout:
    pop  ax
    stc
    ret

; =============================================================================
; Subroutine: read_fdc_byte
; Waits for FDC to have data ready (RQM=1, DIO=1) then reads one byte.
; Destroys: CX, DX
; Returns: AL = byte; carry clear = success, carry set = timeout
; =============================================================================
read_fdc_byte:
    mov  dx, FDC_MSR
    mov  cx, 0             ; 65536 iterations
.loop:
    in   al, dx
    and  al, 0xC0
    cmp  al, 0xC0          ; RQM=1 and DIO=1?
    je   .ready
    loop .loop
    stc
    ret
.ready:
    mov  dx, FDC_DATA
    in   al, dx
    clc
    ret

; =============================================================================
; 512-byte buffer: pre-filled 0x00 at load; overwritten 0xCC at runtime.
; After a successful DMA read from zeroed disk, all bytes should be 0x00.
; =============================================================================
disk_buf:
    times 512 db 0x00
