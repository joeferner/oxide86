# MS-DOS 4.01 Keyboard Interrupt Handling

Analysis based on oxide86 execution trace (oxide86.log) while running the MS-DOS 4.01 setup program.

## Overview

MS-DOS 4.01's `IO.SYS` completely replaces the BIOS INT 09h handler and installs its own
keyboard processing pipeline. Keys do not flow through the standard BIOS INT 09h → BDA
ring buffer path. Instead IO.SYS reads directly from the keyboard port and manages its own
routing, with the INT 15h AH=4Fh keyboard intercept hook deciding whether a key reaches
the BDA buffer.

## Interrupt Vector Replacement

When IO.SYS loads, it replaces IVT[09h] with its own handler at segment `0x1057`, offset
`0x0646`. The original BIOS handler is not chained — IO.SYS takes full ownership of the
keyboard hardware interrupt.

The DOS kernel (`MSDOS.SYS`) lives at segment `0x02C1`. Several other segments observed:
- `0x0070` — keyboard input dispatch / DOS BIOS layer
- `0x1057` — IO.SYS custom INT 09h handler
- `0x17AE`, `0x18FF`, `0x19ED`, `0x1DFE` — application / setup code

## Custom INT 09h Handler (IO.SYS, 0x1057:0646)

The handler structure (reconstructed from trace):

```
1057:0646  jmp  0x064a
1057:064a  push bp, ax, bx, cx, dx, si, di, ds, es
1057:0653  sti                   ; re-enable interrupts
1057:0654  cld
1057:0655  mov  bx, 0x0040
1057:0658  mov  ds, bx           ; point DS at BDA segment

           ; initialise per-keystroke state in CS data
1057:065a  mov cs:[0x0507], 0    ; clear flags
1057:0660  mov cs:[0x0508], 0

           ; set up keyboard ring buffer head/tail in CS data (if not already)
1057:0666  cmp cs:[0x04f4], 0xff
1057:066c  jz  0x0681
1057:066e  mov di, 0x04f5
1057:0671  mov cs:[0x04f0], di
1057:0676  mov cs:[0x04f2], di
1057:067b  mov cs:[0x0505], 0

           ; check configuration word cs:[0x10ea] bits 0x3000
           ; (keyboard type / extended keyboard flags)
1057:0681  test cs:[0x10ea], 0x4200
1057:0688  jnz  0x06d0           ; extended keyboard path (not taken here)
1057:068a  test cs:[0x10ea], 0x0800
1057:0691  jz   0x06a6           ; simplified path — read port 0x60 directly

           ; --- simplified keyboard read path (bits 0x3000 clear) ---
1057:06a6  in   al, 0x60         ; read scan code from keyboard data port
1057:06a8  test cs:[0x10ea], 0x3000
1057:06af  jz   0x06bf           ; bits 0x3000 clear → go to INT 15h intercept path
                                 ; (bits set → full scan code processing with BDA write)

           ; --- INT 15h AH=4Fh keyboard intercept ---
1057:06bf  mov  ah, 0x4f
1057:06c1  stc                   ; calling convention: set CF before call
1057:06c2  int  0x15
1057:06c4  sti
1057:06c5  jb   0x06ca           ; CF=1 → key NOT intercepted → buffer in BDA
1057:06c7  jmp  0x074f           ; CF=0 → key was intercepted → discard

           ; --- 0x06ca: key accepted, add to BDA ring buffer ---
           ; (not observed in trace with old emulator bug)

           ; --- 0x074f: cleanup (reached when key discarded) ---
1057:074f  and  [0x0096], 0xfc   ; clear E0/E1 prefix bits in BDA keyboard status
           ; ... check internal flags ...
1057:076e  cli
1057:076f  mov  al, 0x20
1057:0771  out  0x20, al         ; send EOI to PIC
           ; ... restore registers ...
1057:078f  iret
```

## The INT 15h AH=4Fh Intercept and the Bug

The key decision point is at `0x06c5`: `jb 0x06ca` (jump if carry set).

| CF after INT 15h | Meaning | Branch taken |
|---|---|---|
| CF = 1 | Key NOT intercepted — pass to BDA | `jb 0x06ca` → buffers key |
| CF = 0 | Key intercepted/consumed by hook | `jmp 0x074f` → discards key |

The oxide86 emulator's `int15_keyboard_intercept` was returning **CF=0**, which IO.SYS
interpreted as "some hook consumed this key" and discarded it. The key never reached the
BDA ring buffer, so all subsequent `INT 16h AH=01h` polls returned ZF=1 (no key).

**Fix:** Return CF=1 from `INT 15h AH=4Fh`. This is the correct default semantics per
Ralph Brown's Interrupt List:

> CF set on return → key will be stored in keyboard buffer
> CF clear on return → key will be discarded

The original code comment in the emulator had the semantics backwards.

## Key Polling Path (application side)

The setup program polls for keyboard input via `INT 21h AH=0Bh` (check stdin status),
which routes through the DOS kernel into a dispatch routine in segment `0x0070`:

```
0070:076c  mov  al, [0x061b]     ; check custom input buffer first
0070:076f  or   al, al
0070:0771  jz   0x0776           ; empty → fall through to BIOS check
0070:0776  mov  ah, [0x061d]     ; ah = 0x01
0070:077a  int  0x16             ; INT 16h AH=01h: check BDA keyboard buffer
0070:077c  jz   0x0781           ; ZF=1 → no key available
```

Two sources are checked:
1. A custom input buffer at `0x0070:061Bh`
2. The BIOS BDA ring buffer via `INT 16h AH=01h`

Before the fix, the BDA buffer was always empty because IO.SYS discarded every keystroke.
After the fix, IO.SYS correctly routes non-intercepted keys into the BDA buffer at
`0x06ca`, and the poll above finds them via `INT 16h AH=01h`.

## INT 28h (DOS Idle)

While waiting for input the DOS kernel calls `INT 28h` (the DOS idle hook) on each loop
iteration. The handler at `0x02C1:16FB` is a simple IRET in this trace, but in
multitasking environments (DESQview, etc.) it is hooked to yield CPU time to background
tasks.

## Configuration Word cs:[0x10ea]

IO.SYS maintains a 16-bit flags word at `CS:0x10EA` (physical `0x1165A`). In this trace
the value is `0x8000`:

| Bits | Value | Meaning (inferred) |
|---|---|---|
| 15 (0x8000) | 1 | IO.SYS active / keyboard handler installed |
| 13-12 (0x3000) | 0 | Bits clear → simplified keyboard read path |
| 11 (0x0800) | 0 | Bit clear → read port 0x60 directly (not buffered read) |
| 14,10 (0x4200) | 0 | Bit clear → standard (not extended) keyboard mode |

When bits 0x3000 are set, the handler takes a more complete code path at `0x06b1` that
presumably includes direct BDA ring buffer writes and full scan code processing. When
these bits are clear (as in this trace), IO.SYS relies on INT 15h AH=4Fh returning CF=1
to decide the key should be buffered and then branches to `0x06ca` to do the write.

## Summary of Data Flow (after fix)

```
User presses key
    │
    ▼
keyboard_controller.key_press(scan_code)
    │  pending_key=true, obf=true
    ▼
PIC dispatches IRQ1 → INT 09h
    │  IVT[9] → IO.SYS 0x1057:0646
    ▼
IO.SYS INT 09h handler
    │  in al, 0x60  (reads scan code, clears obf)
    │  stc + int 0x15 AH=4Fh  (keyboard intercept)
    │  BIOS returns CF=1 (not intercepted)  ← fixed
    │  jb 0x06ca  (CF=1, taken)
    ▼
0x06ca: add key to BDA ring buffer
    │  EOI to PIC
    │  iret
    ▼
Application polls INT 16h AH=01h
    │  bda_peek_key() finds key in buffer
    │  ZF=0, AH=scan_code, AL=ascii
    ▼
Keypress detected — setup continues
```
