# Analysis: 'y' Key Not Appearing on Screen

**Date:** 2026-03-04
**Log:** `oxide86.log`
**Symptom:** Pressing 'y' does not display the character on screen.

---

## What the Assembly Is Doing

### Architecture overview

The emulator runs real DOS BIOS code at segment `028E`. When the application at `0DBB` calls `INT 21h AH=0Ah` (buffered keyboard input with echo), the IVT entry points to `028E:1460` — not the emulator's Rust BIOS handler. The full DOS kernel is running as real x86 code.

### Key flow (from the log)

```
0DBB:1247  int 0x21                 ; application requests buffered keyboard input
                                    ; IVT routes to 028E:1460 (DOS handler)

028E:1480  call 0x1580              ; save all registers (custom calling convention)
028E:2060  ...                      ; INT 21h AH=0Ah dispatcher starts
028E:5753  cmp ss:[0x02cf], 0x01    ; check reentrancy counter
028E:577C  call 0x4f93              ; begin key-read/echo sequence
028E:4E49  call 0x4f90              ; dispatch to BIOS key-read function (code 5)
028E:4FCE  call far ss:[0x0320]     ; call 0070:0617 (key read routine)

0070:0617  ...                      ; DOS BIOS key reader
0070:0770  int 0x16                 ; INT 16h AH=01 (peek — is a key available?)
           ; 'y' was already read by INT 16h AH=00 before this log point
0070:06C4  mov [bx+0x03], ax        ; store AX=0x0179 (scan=0x01, ascii=0x79='y')
                                    ; into BIOS data structure at 028E:0327
0070:06D0  retf                     ; return to 028E:4FD2

028E:4FD5  ret                      ; return to 028E:4E4C (dispatcher)
028E:4E4C  ss: mov di, ss:[0x0327]  ; read stored key result (0x0179)
...
028E:4E6F  call 0x156d              ; restore registers (inverse of 0x1580)
028E:4E72  ss: mov ax, ss:[0x035e]  ; load function return value
028E:4E76  ret

           ; *** GAP: INT 10h AH=0Eh fires here (not logged, handled by Rust BIOS) ***

028E:2424  pop dx                   ; resume after echo
028E:2425  pop ax                   ; AX restored (0x0C00)
028E:2426  mov ah, al               ; AL=0x00 — not 'y'
028E:2424–243E  ...                 ; character classification (special char check)
028E:243E  ret

028E:1544  cli                      ; cleanup
028E:156C  iret                     ; return to 0DBB:1242 (re-executes INT 21h setup)
```

The IRET at `028E:156C` returns to `0DBB:1242` (not `0DBB:1249`), causing the application to **re-execute** `mov dx, 0x0379; mov ah, 0Ah; int 0x21`. This is intentional — the DOS AH=0Ah handler loops per-character, returning after each keypress. In subsequent iterations, the code waits at `INT 0x28` (DOS idle) since no more keys are queued.

---

## Issue 1: Double Log Entries for Segment-Prefixed Instructions

Every instruction with a segment override prefix (CS:, SS:, DS:, ES:) appears **twice** in the log:

```
028E:4E4C 36 8B 3E 27 03  ss: mov di, ss:[0x0327]  [0x0327]=0179 @028E:0327  ← correct
028E:4E4D 8B 3E 27 03    mov di, [0x0327]           [0x0327]=2020 @0070:0327  ← wrong!
```

**Root cause:** In `mod.rs`, the segment prefix handler calls `self.step(bus)` recursively:

```rust
0x36 => {
    self.segment_override = Some(self.ss);
    self.step(bus);          // ← inner step logs instruction using fresh Decoder
    self.segment_override = None;
}
```

The outer `step()` decodes the full instruction (including prefix) and shows correct segment values. The inner `step()` creates a **new `Decoder`** that does not inherit `cpu.segment_override`, so it displays DS-based memory addresses for its log output.

**Actual execution is correct** — `cpu.segment_override` IS set when the inner step executes the instruction. The data shown in the second log line is misleading but harmless. However, the second log line can be confusing when diagnosing memory access issues.

### Examples of misleading inner-step log entries

| Outer (correct) | Inner (misleading) |
|---|---|
| `ss:[028E:0327] = 0x0179` | `[0070:0327] = 0x2020` |
| `ss:[028E:035E] = 0x0400` | `[028E:035E] = 0x0100` |
| `ss: call far ss:[028E:0320] = 0x060C` | `call far [0070:0320] = 0x5300` |

---

## Issue 2: Teletype Output Missing Attribute Byte

The echo of 'y' fires via **INT 10h AH=0Eh** (teletype output). This routes to `BIOS_CODE_SEGMENT` (`0xF000`) and is handled by the Rust function `int10_teletype_output`. Because it goes through the BIOS code segment, **no x86 instructions are logged** for it — this explains the gap between `028E:4E76 ret` and `028E:2424 pop dx`.

The implementation in `core/src/cpu/bios/int10_video_services.rs:477`:

```rust
// Text mode: write character byte directly
let offset = (cursor.row as usize * columns as usize + cursor.col as usize) * 2;
bus.memory_write_u8(CGA_MEMORY_START + offset, ch);
// TODO     // Preserve existing color, but substitute 0x07 for 0x00 (black on black)
// TODO     // since text with attribute 0x00 is always invisible.
// TODO     let existing_attr = bus.video().read_byte(offset + 1);
// TODO     if existing_attr == 0x00 {
// TODO         bus.video_mut().write_byte(offset + 1, 0x07);
// TODO     }
```

Only the **character byte** is written. The **attribute byte** (at `offset + 1`) is not touched.

**If the attribute byte at the cursor position is `0x00`** (black foreground on black background), the character is rendered invisibly. The TODO comment acknowledges this and has the fix ready but commented out.

### When does attribute 0x00 occur?

- Screen positions that were never written with a proper attribute
- Areas cleared by code that used attribute `0x00` rather than `0x07`
- Positions where only the character byte was previously written (recursive problem)

### The fix (already identified in the TODO)

```rust
let existing_attr = bus.memory_read_u8(CGA_MEMORY_START + offset + 1);
if existing_attr == 0x00 {
    bus.memory_write_u8(CGA_MEMORY_START + offset + 1, 0x07);
}
```

Note: `scroll_up()` correctly uses `attr: 0x07` for blank lines, so scrolled regions should be fine. The issue is more likely in areas never scrolled through or initialized by a different path.

---

## Summary

| # | Finding | Severity | File |
|---|---|---|---|
| 1 | Double log entries for segment-prefixed instructions (inner step Decoder doesn't inherit `cpu.segment_override`) | Low — cosmetic log issue, execution correct | `core/src/cpu/mod.rs`, `core/src/cpu/instructions/decoder.rs` |
| 2 | `int10_teletype_output` does not write attribute byte; attribute `0x00` makes characters invisible | **High — causes 'y' not to appear** | `core/src/cpu/bios/int10_video_services.rs:477` |

### Custom 1580/156D calling convention

The DOS BIOS uses a non-standard call/return mechanism throughout:
- **`call 0x1580`** — saves return address into `cs:[0x0580]`, pushes 9 registers (ax, bx, cx, dx, si, di, bp, ds, es), jumps to the handler
- **`call 0x156D`** — saves *its own* return address into `cs:[0x0580]`, pops 9 registers (restoring caller context), then `jmp cs:[0x0580]` to continue

This is how the DOS BIOS re-enters the caller's stack frame after completing a sub-function — similar to a trampoline. Understanding this is essential when reading the log, as it makes normal call/ret analysis break down.
