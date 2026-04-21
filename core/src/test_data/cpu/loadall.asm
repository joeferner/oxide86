[CPU 286]
[ORG 0x100]

; 286 LOADALL (0F 05) test
;
; Test 1: Standard case.
;   Builds the 102-byte LOADALL table at physical 0x0800 with CS/IP pointing
;   to after_loadall.  CS cache base = CS*16 (normal real-mode value).
;   Verifies all seven GP register sentinels after LOADALL.
;
; Test 2: Cache-base override.
;   Sets CS_selector = 0x0000 and IP = 0x0000 in the table, but writes
;   cs_cache.base = physical address of after_loadall2.  On a real 286, and
;   after the emulator fix, the CPU resumes at cs_cache.base + IP regardless
;   of the real-mode cs*16 formula.  This is how the SVARDOS XMS driver
;   returns from extended memory.
;
; LOADALL table layout at physical 0x800 (per dosmid XMS / emulator implementation):
;   +0x00  MSW (CR0)
;   +0x02  reserved (20 bytes)
;   +0x16  TR selector
;   +0x18  FLAGS
;   +0x1A  IP  (must point to after_loadall)
;   +0x1C  LDTR selector
;   +0x1E  DS  +0x20  SS  +0x22  CS  +0x24  ES
;   +0x26  DI  +0x28  SI  +0x2A  BP  +0x2C  SP
;   +0x2E  BX  +0x30  DX  +0x32  CX  +0x34  AX
;   +0x36  ES cache (6)  +0x3C  CS cache (6)
;   +0x42  SS cache (6)  +0x48  DS cache (6)
;   +0x4E  GDT (6)  +0x54  IDT (6)
;   +0x5A  LDTR cache (6)  +0x60  TR cache (6)
;
; Descriptor cache entry format (6 bytes):
;   base_lo(1), base_mid(1), base_hi(1), limit[15:0](2), access(1)
;
; Access bytes used:
;   0x93 = present, ring 0, data, writable (DS/SS/ES)
;   0x9B = present, ring 0, code, readable (CS)

section .text
start:
    ; Set ES = 0 so [es:xxxx] reaches physical memory from 0x0000
    xor ax, ax
    mov es, ax

    ; Zero the entire table (enough words to cover up to +0x66)
    mov di, 0x0800
    mov cx, 51          ; 51 words = 102 bytes
    xor ax, ax
    cld
    rep stosw

    ; ---- MSW: real mode (PE=0) ----
    mov word [es:0x0800], 0x0000

    ; ---- System selectors (all null) ----
    ; TR at +0x16, LDTR at +0x1C already zeroed

    ; ---- FLAGS: preserve current flags ----
    pushf
    pop ax
    mov word [es:0x0818], ax

    ; ---- IP: must jump to after_loadall ----
    mov word [es:0x081A], after_loadall

    ; ---- Segment registers: keep current values ----
    mov ax, ds
    mov word [es:0x081E], ax   ; DS
    mov ax, ss
    mov word [es:0x0820], ax   ; SS
    mov ax, cs
    mov word [es:0x0822], ax   ; CS
    mov ax, ds                 ; restore ES = DS after LOADALL
    mov word [es:0x0824], ax   ; ES

    ; ---- General-purpose register sentinels ----
    ; LOADALL will set these exact values into the CPU registers
    mov word [es:0x0826], 0xAA11  ; DI
    mov word [es:0x0828], 0xBB22  ; SI
    mov word [es:0x082A], 0xCC33  ; BP
    ; SP: preserve current stack pointer
    mov word [es:0x082C], sp      ; SP
    mov word [es:0x082E], 0xDD44  ; BX
    mov word [es:0x0830], 0xEE55  ; DX
    mov word [es:0x0832], 0xFF66  ; CX
    mov word [es:0x0834], 0x1234  ; AX

    ; ---- Descriptor caches ----
    ; Format per entry: base_lo(1) base_mid(1) base_hi(1) limit(2) access(1)
    ; For real-mode segment S: base = S*16, limit = 0xFFFF

    ; ES cache at +0x36 (use DS segment value)
    mov ax, ds
    mov cl, 0x93
    mov di, 0x0836
    call build_cache

    ; CS cache at +0x3C
    mov ax, cs
    mov cl, 0x9B
    mov di, 0x083C
    call build_cache

    ; SS cache at +0x42
    mov ax, ss
    mov cl, 0x93
    mov di, 0x0842
    call build_cache

    ; DS cache at +0x48
    mov ax, ds
    mov cl, 0x93
    mov di, 0x0848
    call build_cache

    ; GDT pseudo-descriptor at +0x4E: leave zeroed (limit=0, base=0)
    ; IDT pseudo-descriptor at +0x54: BIOS real-mode IVT (base=0, limit=0x03FF)
    mov word [es:0x0854], 0x03FF  ; IDT limit
    ; IDT base = 0x0000 (already zeroed)

    ; LDTR and TR caches at +0x5A, +0x60: already zeroed

    ; ===== Execute LOADALL (Test 1) =====
    db 0x0F, 0x05

after_loadall:
    ; At this point all GP registers hold the table values.
    ; Verify each sentinel.

    cmp ax, 0x1234
    jne test_fail

    cmp bx, 0xDD44
    jne test_fail

    cmp cx, 0xFF66
    jne test_fail

    cmp dx, 0xEE55
    jne test_fail

    cmp si, 0xBB22
    jne test_fail

    cmp di, 0xAA11
    jne test_fail

    cmp bp, 0xCC33
    jne test_fail

    ; ===== Test 2: cs_cache.base override =====
    ;
    ; Set CS=0 and IP=0 in the table but point cs_cache.base at the physical
    ; address of after_loadall2.  On real 286 (and with the emulator fix) the
    ; CPU always uses cs_cache.base for address translation, so execution must
    ; resume at after_loadall2 rather than at physical 0x00000.

    ; ES was restored to DS by Test 1; re-zero it for direct physical writes
    xor ax, ax
    mov es, ax

    ; Zero the 102-byte table
    mov di, 0x0800
    mov cx, 51
    rep stosw

    ; FLAGS
    pushf
    pop ax
    mov word [es:0x0818], ax

    ; IP = 0 and CS = 0 (already zeroed) — deliberate trap:
    ; if the emulator uses cs*16+ip it will jump to the IVT (physical 0)

    ; Segment registers: keep DS, SS; restore ES = DS after LOADALL
    mov ax, ds
    mov word [es:0x081E], ax    ; DS
    mov ax, ss
    mov word [es:0x0820], ax    ; SS
    ; CS = 0 at +0x22 (already zero)
    mov ax, ds
    mov word [es:0x0824], ax    ; ES

    ; SP
    mov word [es:0x082C], sp

    ; CS cache at +0x3C: base = physical address of after_loadall2
    ; Format: base_lo(1), base_mid(1), base_hi(1), limit(2), access(1)
    ; physical = (cs * 16) + after_loadall2_offset
    mov bx, cs
    mov dx, bx
    shr dx, 12              ; dx = base_hi byte (bits 19-16)
    shl bx, 4               ; bx = (cs << 4) & 0xFFFF
    add bx, after_loadall2  ; add label offset; may carry into dx
    adc dx, 0
    mov byte [es:0x083C], bl        ; cs_cache.base_lo
    mov byte [es:0x083D], bh        ; cs_cache.base_mid
    mov byte [es:0x083E], dl        ; cs_cache.base_hi
    mov word [es:0x083F], 0xFFFF    ; cs_cache.limit
    mov byte [es:0x0841], 0x9B      ; cs_cache.access

    ; Other caches (normal real-mode values)
    mov ax, ds
    mov cl, 0x93
    mov di, 0x0836
    call build_cache    ; ES cache

    mov ax, ss
    mov cl, 0x93
    mov di, 0x0842
    call build_cache    ; SS cache

    mov ax, ds
    mov cl, 0x93
    mov di, 0x0848
    call build_cache    ; DS cache

    ; IDT pseudo-descriptor (real-mode IVT: base=0, limit=0x03FF)
    mov word [es:0x0854], 0x03FF

    ; ===== Execute LOADALL (Test 2) =====
    db 0x0F, 0x05

    ; If the emulator still uses cs*16+ip (=0) it will fetch from the IVT and
    ; the test will not exit cleanly.  With the fix it lands here:
after_loadall2:
    mov ax, 0x4C00
    int 21h

test_fail:
    mov ax, 0x4C01
    int 21h

;=============================================================================
; build_cache: write a 6-byte real-mode descriptor cache entry
; In:  AX = segment value, CL = access byte, ES:DI = destination
; Format: base_lo(1), base_mid(1), base_hi(1), limit(2), access(1)
; Out: (nothing; clobbers AX, BX)
;=============================================================================
build_cache:
    ; base = segment * 16 (24-bit)
    ; base_lo  = (seg << 4) & 0xFF
    ; base_mid = (seg >> 4) & 0xFF
    ; base_hi  = (seg >> 12) & 0xFF

    mov bx, ax              ; save segment

    shl ax, 4               ; ax = (seg << 4) & 0xFFFF
    mov byte [es:di], al    ; base_lo
    mov byte [es:di+1], ah  ; base_mid

    mov ax, bx
    shr ax, 12
    mov byte [es:di+2], al  ; base_hi

    mov word [es:di+3], 0xFFFF  ; limit
    mov byte [es:di+5], cl      ; access byte

    mov ax, bx              ; restore ax = original segment
    ret
