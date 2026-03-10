# BIOS Options Analysis for oxide86

## Background

oxide86 currently uses a "magic dispatch" approach for BIOS services: when the CPU
executes with `CS=0xF000` and `IP<=0xFF`, instead of executing x86 machine code the
emulator intercepts control and calls Rust handlers directly. This works well but is
not how real hardware behaves.

The question was: could a real BIOS ROM (SeaBIOS, 8086tiny's BIOS, etc.) replace the
custom Rust interrupt handlers?

---

## Current Architecture

- **IVT**: All 256 entries point to `F000:0x00`–`F000:0xFF` (one per interrupt number).
- **Dispatch**: `cpu/mod.rs` detects `CS=0xF000, IP<=0xFF` and calls the matching Rust handler.
- **No ROM code**: The ROM area holds fonts (`F000:B000`, `F000:C000`) and the INT 15h
  system config table (`F000:E000`), but no executable BIOS machine code.
- **Handlers**: 13 interrupt handlers in `core/src/cpu/bios/` (INT 08–1A, 21, 74).

---

## Option 1: SeaBIOS

SeaBIOS is the open-source BIOS used by QEMU. It is real x86 machine code that runs on
the emulated CPU.

### Requirements

| Requirement | oxide86 status |
|---|---|
| 32-bit protected mode (used during POST) | Not implemented |
| PCI bus (device discovery) | Not implemented |
| Accurate PIC/PIT/CMOS behavior | Partially — simplified |
| VGA option ROM (INT 10h delegated separately) | Not present |
| ROM loaded at `F000:E000`, reset vector at `FFFF:0000` | Magic dispatch used instead |

### Verdict: Not practical

SeaBIOS requires protected mode support just to complete its POST initialization, before
it can provide any legacy BIOS services. It also expects a PCI bus and a separate VGA
BIOS ROM. Adopting it would require implementing 32-bit protected mode, a PCI bus, and
much higher hardware fidelity — a multi-step project well beyond a drop-in replacement.

---

## Option 2: 8086tiny BIOS

[8086tiny](https://github.com/adriancable/8086tiny) is a tiny PC XT emulator that ships
its own custom BIOS written from scratch in x86 assembly (~6KB, assembled with NASM).

### How it works

- The BIOS binary is loaded at `F000:0100` in the emulator.
- Standard interrupt handlers (INT 10h video, INT 13h disk, INT 1Ah time, etc.) are
  implemented as real 8086 assembly code.
- For operations that cannot be expressed in pure x86 (terminal output, RTC reads, disk
  I/O), the BIOS uses **special 2-byte escape opcodes** (`0F 00`–`0F 03`) that the
  emulator intercepts and handles in C.

### Comparison

| | SeaBIOS | 8086tiny BIOS | oxide86 (current) |
|---|---|---|---|
| Code | Real x86 binary | Real x86 binary | Rust magic dispatch |
| Protected mode required | Yes | No | No |
| PCI bus required | Yes | No | No |
| Escape opcodes | No | Yes (`0F 00`–`0F 03`) | N/A |
| Target hardware | Modern legacy PC | PC XT | PC XT/AT |
| Size | ~128KB | ~6KB | — |

### Could oxide86 use it directly?

Partially, but there are mismatches:

- **Video**: 8086tiny renders CGA/Hercules to ANSI terminal escape sequences. oxide86
  has a proper VGA emulation; the INT 10h handlers would be incompatible.
- **Escape opcodes**: oxide86 would need to implement `0F 00`–`0F 03` in its CPU decoder
  to bridge x86 BIOS calls back into Rust hardware layer.
- **Disk interface**: The disk escape uses 8086tiny's own calling conventions, not
  oxide86's `Device` trait.

### Verdict: More feasible than SeaBIOS, but still significant adaptation work

---

## Key Insight from 8086tiny

8086tiny demonstrates a viable middle path: **a small custom x86 BIOS binary combined
with a handful of escape opcodes** to call back into native code for hardware I/O.

This maps cleanly onto oxide86's existing architecture:

```
Real x86 BIOS code in ROM
        │
        │  (INT handlers, scan code tables, BDA setup — all in 8086 assembly)
        │
        ▼
Escape opcode (e.g. 0F 01)
        │
        │  (CPU decoder intercepts, calls Rust)
        ▼
Device trait (PIC, PIT, keyboard, disk, VGA ...)
```

This is essentially what oxide86 already does — just with Rust magic instead of real x86
code in ROM.

---

## Recommendation

**Keep the current Rust BIOS approach.** It is simpler, faster to iterate on, and
requires no protected mode or PCI emulation. The existing 13 interrupt handlers cover
what DOS software needs.

If moving toward a real BIOS ROM becomes desirable in the future, the recommended path
is:

1. Implement 32-bit protected mode (needed by most real BIOSes).
2. Write or adapt a small custom BIOS in x86 assembly (following the 8086tiny model).
3. Define a small set of escape opcodes that bridge x86 BIOS code → Rust `Device` trait.
4. Load the binary into the ROM area (`0xF0000`–`0xFFFFF`) and start the CPU at the
   real reset vector (`FFFF:0000`).

This staged approach avoids the complexity of SeaBIOS while preserving the option to
run real BIOS code eventually.
