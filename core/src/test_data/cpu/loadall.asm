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
; LOADALL table layout at physical 0x800:
;   +0x00  MSW (CR0)
;   +0x02  reserved (14 bytes)
;   +0x10  TR selector
;   +0x12  FLAGS
;   +0x14  IP  (must point to after_loadall)
;   +0x16  LDTR selector
;   +0x18  DS  +0x1A  SS  +0x1C  CS  +0x1E  ES
;   +0x20  DI  +0x22  SI  +0x24  BP  +0x26  SP
;   +0x28  BX  +0x2A  DX  +0x2C  CX  +0x2E  AX
;   +0x30  ES cache (6)  +0x36  CS cache (6)
;   +0x3C  SS cache (6)  +0x42  DS cache (6)
;   +0x48  GDT (6)  +0x4E  IDT (6)
;   +0x54  LDTR cache (6)  +0x5A  TR cache (6)
;
; Descriptor cache entry format (6 bytes):
;   limit[15:0](2), base[15:0](2), base[23:16](1), access(1)
;
; Access bytes used:
;   0x93 = present, ring 0, data, writable (DS/SS/ES)
;   0x9B = present, ring 0, code, readable (CS)

section .text
start:
    ; Set ES = 0 so [es:xxxx] reaches physical memory from 0x0000
    xor ax, ax
    mov es, ax

    ; Zero the entire 102-byte table first
    mov di, 0x0800
    mov cx, 51          ; 51 words = 102 bytes
    xor ax, ax
    cld
    rep stosw

    ; ---- MSW: real mode (PE=0) ----
    mov word [es:0x0800], 0x0000

    ; ---- System selectors (all null) ----
    ; TR at +0x10, LDTR at +0x16 already zeroed

    ; ---- FLAGS: preserve current flags ----
    pushf
    pop ax
    mov word [es:0x0812], ax

    ; ---- IP: must jump to after_loadall ----
    mov word [es:0x0814], after_loadall

    ; ---- Segment registers: keep current values ----
    mov ax, ds
    mov word [es:0x0818], ax   ; DS
    mov ax, ss
    mov word [es:0x081A], ax   ; SS
    mov ax, cs
    mov word [es:0x081C], ax   ; CS
    mov ax, ds                 ; restore ES = DS after LOADALL
    mov word [es:0x081E], ax   ; ES

    ; ---- General-purpose register sentinels ----
    ; LOADALL will set these exact values into the CPU registers
    mov word [es:0x0820], 0xAA11  ; DI
    mov word [es:0x0822], 0xBB22  ; SI
    mov word [es:0x0824], 0xCC33  ; BP
    ; SP: preserve current stack pointer
    mov word [es:0x0826], sp      ; SP
    mov word [es:0x0828], 0xDD44  ; BX
    mov word [es:0x082A], 0xEE55  ; DX
    mov word [es:0x082C], 0xFF66  ; CX
    mov word [es:0x082E], 0x1234  ; AX

    ; ---- Descriptor caches ----
    ; Format per entry: limit(2) base_low(2) base_high(1) access(1)
    ; For real-mode segment S: base = S*16, limit = 0xFFFF

    ; Helper: AX = segment, build cache at ES:DI, CL = access byte
    ; We build 4 caches in sequence: ES, CS, SS, DS

    ; ES cache at +0x30 (use DS segment value)
    mov ax, ds
    mov cl, 0x93
    mov di, 0x0830
    call build_cache

    ; CS cache at +0x36
    mov ax, cs
    mov cl, 0x9B
    mov di, 0x0836
    call build_cache

    ; SS cache at +0x3C
    mov ax, ss
    mov cl, 0x93
    mov di, 0x083C
    call build_cache

    ; DS cache at +0x42
    mov ax, ds
    mov cl, 0x93
    mov di, 0x0842
    call build_cache

    ; GDT pseudo-descriptor at +0x48: leave zeroed (limit=0, base=0)
    ; IDT pseudo-descriptor at +0x4E: BIOS real-mode IVT (base=0, limit=0x03FF)
    mov word [es:0x084E], 0x03FF  ; IDT limit
    ; IDT base = 0x0000 (already zeroed)

    ; LDTR and TR caches at +0x54, +0x5A: already zeroed

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
    mov word [es:0x0812], ax

    ; IP = 0 and CS = 0 (already zeroed) — deliberate trap:
    ; if the emulator uses cs*16+ip it will jump to the IVT (physical 0)

    ; Segment registers: keep DS, SS; restore ES = DS after LOADALL
    mov ax, ds
    mov word [es:0x0818], ax    ; DS
    mov ax, ss
    mov word [es:0x081A], ax    ; SS
    ; CS = 0 at +0x1C (already zero)
    mov ax, ds
    mov word [es:0x081E], ax    ; ES

    ; SP
    mov word [es:0x0826], sp

    ; CS cache at +0x36: base = physical address of after_loadall2
    ;   physical = (cs * 16) + after_loadall2_offset
    ;   base_high = cs >> 12  (upper 8 bits of 24-bit base)
    ;   base_low  = (cs << 4) & 0xFFFF + after_loadall2_offset (+ carry)
    mov ax, cs
    mov dx, ax
    shr dx, 12              ; dx = base_high byte
    shl ax, 4               ; ax = (cs << 4) & 0xFFFF
    add ax, after_loadall2  ; add label offset; may carry into dx
    adc dx, 0
    mov word [es:0x0836], 0xFFFF    ; cs_cache.limit
    mov word [es:0x0838], ax        ; cs_cache.base_low
    mov byte [es:0x083A], dl        ; cs_cache.base_high
    mov byte [es:0x083B], 0x9B      ; cs_cache.access

    ; Other caches (normal real-mode values)
    mov ax, ds
    mov cl, 0x93
    mov di, 0x0830
    call build_cache    ; ES cache

    mov ax, ss
    mov cl, 0x93
    mov di, 0x083C
    call build_cache    ; SS cache

    mov ax, ds
    mov cl, 0x93
    mov di, 0x0842
    call build_cache    ; DS cache

    ; IDT pseudo-descriptor (real-mode IVT: base=0, limit=0x03FF)
    mov word [es:0x084E], 0x03FF

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
; Out: (nothing; clobbers AX, BX)
;=============================================================================
build_cache:
    ; limit = 0xFFFF
    mov word [es:di], 0xFFFF

    ; base = segment * 16
    ; base[15:0] = (AX << 4) — computed as AX*16, lower 16 bits
    mov bx, ax
    shl ax, 4
    mov word [es:di+2], ax   ; base_low

    ; base[23:16] = (original_segment >> 12) — upper nibble
    mov ax, bx
    shr ax, 12
    mov byte [es:di+4], al   ; base_high

    mov byte [es:di+5], cl   ; access byte
    ret
