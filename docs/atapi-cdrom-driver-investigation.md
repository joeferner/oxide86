# ATAPI CD-ROM Driver Investigation

## Overview

This document records the findings from debugging ATAPI CD-ROM detection in emu86.
The goal is to understand why `atapicd.sys` (ATAPI CD/DVD-ROM Device Driver Version 2.12)
reports "CD/DVD-ROM drive not ready." even after the ATA bus scan successfully finds
an ATAPI CD-ROM device.

---

## Driver Being Tested

- **Binary**: `atapicd.sys` inside `/home/fernejo/dos/cdrom.img`
- **Loaded at segment**: `026F:0000`
- **Header**: `.SYS` device driver with device name `CD003   ` (8 chars)
- **Strategy routine**: `026F:1CF3`
- **Interrupt/request handler**: `026F:1D02`
- **Driver banner**: `ATAPI CD/DVD-ROM Device Driver Version 2.12`

The binary in `/home/fernejo/dos/atapicd/atapicd.sys` is a **different version**
from the one in `cdrom.img` — confirmed by byte mismatches at offset 0x1B9
(file: `0x76`, runtime log: `0x06`). All analysis below uses the runtime log.

---

## Fixes Applied (Previous Sessions)

### Fix 1: ATAPI-4 IDENTIFY DEVICE Compliance

**Problem**: When driver sends IDENTIFY DEVICE (`0xEC`) to an ATAPI device, the
emulator returned ABRT (old ATAPI-1 behavior). Windows 9x-era drivers require
ATAPI-4 behavior: return DRQ=1 with 512 bytes of IDENTIFY PACKET DEVICE data.

**Fix** (`core/src/cpu/bios/ata.rs` — `ata_cmd_identify()`):
```rust
// CdRom arm: call ata_cmd_identify_packet() instead of ABRT
if matches!(device, AtaDeviceType::CdRom(_)) {
    self.ata_cmd_identify_packet(device);
} else {
    self.shared.ata_primary.set_error_bits(ata::error::ABRT);
}
```

**Result**: Driver now reads 512 bytes (status = `0x48` = DRQ|DRDY), prints the
version banner, and proceeds to ATA channel scanning.

---

## Driver Initialization Flow

```
DOS calls INIT command
  → 026F:1D02 (interrupt handler)
    → 026F:5E66 (init command handler)
      → print banner
      → INT 13h AH=08h (get hard drive count)
      → call 0x5C39 (setup: read ATAPI device information)
      → call 0x2B0F (main init function)
        → call 0x3C2A (ATA channel scanner)
          → scan primary   (0x1F0): finds HDD master + CD-ROM slave ✓
          → scan secondary (0x170): no devices — 9-tick BDA timeout × many attempts
          → scan tertiary A (0x1E8): no devices — 9-tick BDA timeout × many attempts
          → scan tertiary B (0x168): no devices — 9-tick BDA timeout × many attempts
          → call 0x3BDC ← CRITICAL — tries to open "PCMICLAS" via INT 21h AH=3Dh
            → open FAILS (CF=1)
            → returns AX=0
        → 0x3C9A: or ax, ax → AX=0 → jz 0x3CC1
        → 0x3CC1: cmp [0x1ae6], 0x0000 → equals → jz 0x3CDA
        → 0x3CDA: returns 0xFFFF ("not ready")
      → print "CD/DVD-ROM drive not ready."
```

**The 2-minute delay** before "not ready" is from scanning three non-existent
ATA channels (secondary, tertiary A, tertiary B). Each channel without a device
causes 9-tick BDA timer waits (9 × 55ms ≈ 495ms) repeated many times per channel.
These return 0x7F (not busy) for status, which the driver interprets as
"no device present."

---

## Key Driver Variables

| Address | Meaning |
|---------|---------|
| `026F:1ae6` | CD-ROM readiness flag. Initialized to `0x0000` at `026F:3C3E`. Must be set to non-zero for driver to report "ready." Never set in current execution. |
| `026F:1afa` | Status port (0x1F7, then changes to 0x177, 0x1EF, 0x16F as channels are scanned) |
| `026F:1afc` | Device/head port (0x1F6, then changes to 0x176, 0x1EE, 0x16E) |
| `026F:0091` | IRQ number (set to `0x76` = IRQ14 for primary ATA) |
| `026F:0081` | Hard drive present flag (set to `0xFFFF` when INT 13h AH=08h succeeds) |

---

## The "PCMICLAS" Mystery

### What happens

Function `0x3BDC` is called after all ATA channel scans complete:

```asm
026F:3BED  mov ax, 0x3d00        ; INT 21h open file, read-only
026F:3BF0  mov dx, 0x009c        ; filename at DS:009C = "PCMICLAS"
026F:3BF3  int 0x21
026F:3BF5  jnb 0x3bfb            ; jump if open SUCCEEDED
026F:3BF7  xor ax, ax            ; open FAILED → AX=0
026F:3BF9  jmp 0x3c26            ; return 0 (failure)
; success path at 0x3BFB: (never reached — see below)
```

- Open FAILS (CF=1 returned by INT 21h)
- `0x3BDC` returns `AX=0`
- Caller at `0x3C9A` sees `AX=0` → jumps to `0x3CC1`
- `[0x1ae6]` is still `0x0000` → driver reports "not ready"

### What "PCMICLAS" is

The string `"PCMICLAS"` is hardcoded in the driver binary at offset `0x9C`
(relative to the driver's load segment). Driver data at `DS:009C`:

```
026F:009C: 50 43 4D 49 43 4C 41 53 00
           P  C  M  I  C  L  A  S  \0
```

The driver's own device name (from the `.SYS` header at offset `0x0A`) is
`"CD003   "` — a different name. So `"PCMICLAS"` is **not** this driver's own name.

### Hypotheses

**Hypothesis A — External dependency**: `PCMICLAS` is another DOS device driver
that must be loaded in `CONFIG.SYS` before `ATAPICD.SYS`. The ATAPI driver uses
IOCTL to communicate with it. If not loaded, CD-ROM fails.

**Hypothesis B — Self-installed second device**: The driver installs a second
device header named `"PCMICLAS"` into the DOS device chain during an earlier
startup phase (possibly via segment fixups observed at `026F:0B97–0BA6`). The
`0x3BDC` open is a verification step. If self-installation failed or was skipped,
verification fails.

The segment fixup code at `026F:0B97`:
```asm
026F:0B90  mov ax, cs            ; AX = 026F (driver segment)
026F:0B92  push es
026F:0B93  mov es, [0x013e]      ; es = 0x0000
026F:0B97  es: add es:[0xa002], ax   ; patch segment at 0x0000:0xA002
026F:0B9C  es: add es:[0xa00c], ax   ; patch segment at 0x0000:0xA00C
026F:0BA1  es: add es:[0xa010], ax   ; patch segment at 0x0000:0xA010
026F:0BA6  pop es
026F:0BA7  mov ah, 0x09
026F:0BA9  mov dx, 0x1d4e
026F:0BAC  int 0x21              ; print string at DS:1D4E
```

The fixups at `0x0000:0xA002`, `0x0000:0xA00C`, `0x0000:0xA010` patch absolute
low-memory addresses with the driver's segment. This suggests the driver placed
a second device header at those addresses during an earlier phase, with segment
words left as `0x0000` (relative), now being relocated to actual segment `026F`.

**Whether this is Hypothesis A or B depends on whether `0x3BFB` (the success path)
does device-specific I/O or just reads already-available data.**

---

## What Happens on the Success Path (0x3BFB)

The success path (`0x3BFB`) has **never been reached** in any log, so we cannot
see it directly. The binary at `/home/fernejo/dos/atapicd/atapicd.sys` does not
match the runtime binary, so direct disassembly from file is unreliable.

To observe it, either:
1. Extract `atapicd.sys` from `cdrom.img` and disassemble at offset `0x3BFB`
2. Artificially succeed the `INT 21h` open (return a fake handle) and observe
   what IOCTL/read calls follow

---

## ATA Channel Scan Details

The scanner at `0x3B7C` takes a base I/O port (e.g., `0x1F0`) and iterates over
master (device select `0xA0`) and slave (device select `0xB0`) for each channel.

After the scan of the primary channel:
- Master: Hard drive (IDENTIFY DEVICE 0xEC succeeds, returns 512 bytes)
- Slave: CD-ROM (IDENTIFY DEVICE 0xEC → handled as ATAPI-4 → returns 512 bytes with word[0]=0x8580)

The CD-ROM was found at log line ~171862. No PACKET commands (0xA0) were ever
sent to the primary ATA channel after this point.

---

## What Is NOT Happening

- No `ATA Command: 0xA0` (PACKET) log entries appear anywhere — the driver never
  reaches the stage where it sends ATAPI CDB commands.
- `[0x1ae6]` is never set to a non-zero value.
- The driver's success path (`0x3BFB` onwards) is never reached.

---

## Next Steps

1. **Extract the actual `atapicd.sys` from `cdrom.img`** and disassemble the
   success path at offset `0x3BFB` to understand what the driver expects from
   `"PCMICLAS"` (IOCTL? read? specific data format?).

2. **Determine if `PCMICLAS` is self-installed**: Check whether the driver,
   during its early startup (`026F:0B90–0BAC`), writes a device header named
   `"PCMICLAS"` into DOS memory such that `INT 21h` can find it.

3. **Check if a companion driver is required**: Some ATAPI drivers require a
   helper driver (e.g., a bus-mastering DMA helper or a PCMCIA card services
   layer). If `PCMICLAS` is such a companion, it would need to be loaded before
   `ATAPICD.SYS` in `CONFIG.SYS`.

4. **Consider emulating `PCMICLAS`**: If the driver's IOCTL to `PCMICLAS` is
   well-understood, the emulator could intercept the `INT 21h` open and return
   a synthetic file handle, then handle subsequent IOCTL calls to provide the
   data the driver expects.
