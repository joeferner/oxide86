# CHECKIT.EXE Crash Investigation

## Goal

CHECKIT.EXE step 7 of 12 fails in oxide86. The program crashes inside the
`91C2` overlay module. This document tracks what we've found and what remains
unexplained.

---

## Environment

| Item | Value |
|------|-------|
| Program | `CHECKIT.EXE` |
| PSP segment | `0x0F54` |
| Load segment | `0x0F64` (PSP + 0x10) |
| Global data segment | `DS = 4033` (throughout execution) |
| Physical DS base | `0x40330` |
| Failing step | Step 7 of 12 |

### EXE Header

| Field | Value |
|-------|-------|
| e_cblp | 0x01C0 (448) |
| e_cp | 0x00BB (187) |
| e_crlc | 0x04B8 (1208 reloc entries) |
| e_cparhdr | 0x0140 (320 paragraphs = 5120-byte header) |
| e_minalloc | 0x5A2A |
| e_maxalloc | 0xFFFF |
| e_ss | 0x763E, e_sp = 0x0020 |
| e_cs | 0x1561, e_ip = 0x0322 |
| Image size | 95680 bytes (header + image = 100800 bytes; overlays start here) |

**DS is in BSS:** DS relative offset from load segment = `0x4033 − 0x0F64 = 0x30CF` paragraphs =
197872 bytes. Image is only 95680 bytes. DS starts 102192 bytes past the end of the image → DS is
entirely in BSS and should be zero-initialised on load.

---

## Crash Chain

1. **`91C2:4D02`** — `test [4033:031E], 0xFF`
   - Physical address `0x4064E`
   - Value is `0x64` (non-zero) → `je 0x4D0C` **NOT taken** → init code skipped
   - Jumps to `jmp 0x79EB` (the "already initialised" path)

2. **`91C2:7917`** — `mov ds, [CS:8D77]`
   - Loads DS from far-pointer stored at physical `0x9A997`
   - Value is `0x75E4` — **garbage** left by early DOS init at `0E10:03D7`
   - DS = `0x75E4` is wrong for everything that follows

3. **`91C2:4C29`** — `call far [CS:8D75]`
   - Calls `75E4:0A02` — executes zeroed memory as code → **crash**

### What the init code at `91C2:4D0C` would have done

If the flag at `[4033:031E]` had been `0x00`, the init code at `4D0C` would
have run and written the correct segment value into `[CS:8D77]` (physical
`0x9A997`), replacing the garbage `0x75E4`. The subsequent call at `4C29`
would then have worked correctly.

---

## Physical Address `0x4064E` — What Writes There

The watchpoint on `0x4064E` fires **exactly once** in the live MCP run: during
the overlay's `AH=3F` disk read. The write comes from `int13_read_sectors` in
the emulator (which uses `bus.memory_write_u8` → properly fires watchpoints).

The reported CS:IP is `0070:079C`, which is the CALL FAR instruction in the DOS
kernel that invokes INT 13h — see [Why watch_cs/watch_ip = 0070:079C](#why-watchcswatch_ip--007007) below.

After the overlay loads and execution reaches `91C2:4D02`, the watchpoint does
**not** fire again. Everything between overlay load and the init check is clean;
the `0x64` at `0x4064E` is exactly what the overlay disk read wrote.

---

## The Root Cause

### Buffer overlap

The overlay is loaded by the Borland overlay manager via `AH=3F` into a 65520-byte
staging buffer:

| Item | Value |
|------|-------|
| Buffer base (DS:DX at AH=3F trap) | segment `39F8`, DX=0, physical `0x39F80` |
| Buffer end | `0x39F80 + 0xFFF0 − 1 = 0x49F6F` |
| DS base | `0x40330` |
| DS base offset from buffer start | `0x40330 − 0x39F80 = 0x63B0` (25520 bytes) |
| `[DS:031E]` offset in buffer | `0x63B0 + 0x031E = 0x66CE` (26318 bytes) |

`0x66CE < 0xFFF0` → the init flag **is inside the overlay load buffer**.

### What the overlay file contains at that position

| File offset | Buffer offset | Value |
|-------------|---------------|-------|
| 301950 (`0x499BE`) | `0x66CE` | `0x64` = `'d'` |

This is the first byte of the format string `"d ram_basebeg\t0x%07lx\n"` in
the overlay's data section. The data section starts at buffer offset `0x66B2`;
the init flag lands 28 bytes in.

File offset 301950 is confirmed by:
- `xxd -s 301950 target/CHECKIT.EXE | head -2` → `64 20 72 61 6d 5f 62 61...`
- Live MCP `read_memory` of physical `0x4063E` → format strings visible at `0x4063E`
- Buffer[0:7] matches file[275632:7] exactly; bytes 8-9 differ by a segment relocation
  (`0x274B + PSP(0x0F54) = 0x369F` → stored as `9F 36` in memory), confirming
  file position tracking is accurate.

### Why this crashes

The overlay data section is loaded into the buffer at `0x39F80`, but that buffer
physically overlaps DS (at `0x40330`). The byte at file offset 301950 (`0x64`) is
written to physical `0x4064E`, which is also `[DS:031E]` — the first-time init flag.

Because the flag is non-zero, the init code that would populate `[CS:8D77]` is
skipped. Later, `MOV DS,[CS:8D77]` loads garbage (`0x75E4`), and the subsequent
far call crashes.

### Why this doesn't happen on real DOS

On real DOS the same EXE is loaded and the same buffer overlap geometry applies.
The most likely explanation is that the requested byte count (CX in the `AH=3F`
call) is **smaller on real DOS** — less than 26318 bytes (`0x66CE`) — so the
data section never reaches `[DS:031E]`. The emulator may be passing a larger CX
(e.g. the full 65520-byte buffer size) where real DOS would pass only the number
of bytes remaining in the overlay.

Alternatively, the buffer may be placed at a different physical address on real
DOS (different `AH=48` allocation result), putting it clear of DS.

CX is not currently logged for AH=3F calls — see [Open Questions](#open-questions).

---

## Why watch_cs/watch_ip = 0070:079C

The watchpoint fires inside the emulator's `int13_read_sectors`, which uses
`bus.memory_write_u8`. At that point `bus.current_ip` contains the last address
set by `bus.set_current_ip(pre_cs, pre_ip)` in `cpu/mod.rs`.

**The BIOS early-return path** (`cpu/mod.rs` line 365):

```rust
if self.cs == BIOS_CODE_SEGMENT {
    self.step_bios_int(bus, self.ip as u8);
    // ... returns at line 408
    return;   // ← returns HERE, before set_current_ip
}
// Only reached for non-BIOS instructions:
let pre_cs = self.cs;
let pre_ip = self.ip;
bus.set_current_ip(pre_cs, pre_ip);   // line 414
self.exec_instruction(bus);
```

When the overlay's AH=3F call causes the DOS kernel to call INT 13h:

1. `0070:079C` = `CALL FAR [CS:00B4]`
2. `CS:00B4` at physical `0x7B4` = `13 00 00 F0` → far pointer `F000:0013`
3. `F000` = `BIOS_CODE_SEGMENT`; `set_current_ip` is never called for the BIOS path
4. `bus.current_ip` stays at `0070:079C` (last non-BIOS instruction)
5. Watchpoint fires → reports `0070:079C` as the writing instruction

The write actually comes from `int13_read_sectors` inside the INT 13h handler,
but the CS:IP attribution is one instruction too early.

---

## Emulator Debugger Bugs Found

### 1. Breakpoint executes the instruction before pausing (on continue) — **FIXED**

`debug_check` is called at the top of `step()` before `cpu.step`. When a breakpoint
fires, `do_pause` blocks, the user sends Continue, `do_pause` returns, and then
`step()` falls through to `cpu.step` — **executing the breakpoint instruction**.

**Fix:** `debug_check` returns `bool`; `step()` skips `cpu.step` when `true`.

### 2. Write watchpoint CS:IP reported one instruction late — **FIXED**

`memory_write_u8` stores `(addr, val, 0, 0)` in `watchpoint_hit` and sets
`pause_requested`. The CS:IP is filled in by `do_pause` on the *next* iteration —
by which time IP has already advanced. The MCP watchpoint paused at `0070:07A1`
instead of the actual writing instruction.

**Fix:** Capture `watch_cs / watch_ip` at write time in `bus.rs` instead of
deferring to `do_pause`.

Both fixes implemented in `core/src/computer.rs` and `core/src/bus.rs`.

### 3. BIOS path skips set_current_ip — **not yet fixed**

See [Why watch_cs/watch_ip = 0070:079C](#why-watchcswatch_ip--007007) above.
`bus.set_current_ip` should also be called for BIOS dispatches so watchpoint
attribution is correct even during INT 13h execution.

---

## Confirmed Facts (from live MCP session + log analysis)

### Overlay read sizes — CONFIRMED

From the actual log (`oxide86.log` lines 1861–1865):

```
AH=3F read "CHECKIT.EXE" pos=210112 cx=65520 → phys 0x29F90-0x39F7F (65520 bytes)
AH=3F read "CHECKIT.EXE" pos=275632 cx=65520 → phys 0x39F80-0x49F6F (65520 bytes)
AH=3F read "CHECKIT.EXE" pos=341152 cx=15664 → phys 0x49F70-0x4DC9F (15664 bytes)
```

- CX = 65520 for the first two reads, **15664** for the third.
- Overlay data covers physical `0x29F90`–`0x4DC9F`. The third read ends at
  `0x4DC9F`, well **below** `0x50330` (the MCB boundary after CHECKIT's block).
- The corrupting byte (`0x64` at `0x4064E`) is written by the **second** read
  (`0x39F80`–`0x49F6F`), as previously confirmed.

### CHECKIT accesses memory beyond its MCB

`CHECKIT.CNF` is read into a buffer at physical `0x5034C` (= segment `0x5033`,
offset `0x001C`). This is **28 bytes past** CHECKIT's MCB end (`0x5032F`).
CHECKIT freely accesses this region at runtime; DOS has no memory protection.

This means whatever is at segment `0x5033` (physical `0x50330`) is overwritten
by CHECKIT's runtime use of that memory. The `0xFE` byte seen there during the
live MCP session is **not** an original MCB byte — it was written by CHECKIT
after load.

### CHECKIT self-shrinks via AH=4A — CONFIRMED

CHECKIT gets the full available free block at EXEC time (PSP:0002=`0xA000`, ~590 KB).
Immediately on entry, the Borland overlay manager at `19CD:000E`–`19CD:0064`
(log lines 854661–854689) computes:

```
DI = DS = 0x4033
SI = [PSP:0002] = 0xA000
SI = 0xA000 - 0x4033 = 0x5FCD   ; paragraphs available above DS
SI capped to 0x1000              ; Borland OVR limits its "arena" to 64 KB
new_top = SI + DI = 0x1000 + 0x4033 = 0x5033
[PSP:0002] = 0x5033              ; update PSP before the call
BX = 0x5033 - PSP(0x0F44) = 0x40EF
INT 21h AH=4A, ES=0x0F44, BX=0x40EF   ; shrink CHECKIT's block
```

The `0x0021`-paragraph allocation from the free block at `0x5033` (log line 915672)
is a subsequent small allocation by the overlay manager — likely a control structure.

No `AH=48` calls occur after CHECKIT loads. The overlay buffer addresses
(`0x29F9`, `0x39F8`) are set purely by EXE relocation at load time; the Borland
OVR uses static pre-allocated buffers within BSS, not dynamically allocated ones.

---

### Memory layout is identical for 286 and 8086 CPU modes

In both CPU modes, COMMAND.COM's AUTOEXEC.BAT buffer lands at physical `0x9FFAF`
(confirmed in both logs). The DOS kernel, COMMAND.COM, and CHECKIT load at the
same addresses regardless of CPU type. CHECKIT's self-shrink to `0x40EF` paragraphs ≈ 260 KB (Borland OVR AH=4A) is
**not** caused by the CPU mode — it happens identically in both modes.

### MCB chain — what exists above CHECKIT's block — REVISED

The MCB at `0x5033` is **not** COMMAND.COM's transient. It is the free remainder
created when CHECKIT's own Borland overlay manager shrinks CHECKIT's allocation
(see "CHECKIT self-shrinks via AH=4A" below).

- Initial free block at `0x0F43`: size `0x90BC` paragraphs (owner = 0, type `'Z'`),
  created by COMMAND.COM's AH=4B EXEC handler. `0x0F44 + 0x90BC = 0xA000` → full
  640 KB available.
- CHECKIT's OVR at `19CD:0064` calls `AH=4A` (BX=`0x40EF`, ES=`0x0F44`) to shrink
  CHECKIT's block. The allocator writes MCB size `0x40EF` at `0x0F43:0003`, then
  creates a **free** remainder MCB at `0x5033` with size `0x4FCC`, type `'Z'`,
  owner `0` (log lines 854820–854825).
- Subsequent overlay-manager AH=4A at log line 915672 allocates `0x0021` paragraphs
  from the free block at `0x5033` (owner set to `0x0F44`), leaving a free block at
  `0x5055`.

Since the overlays do NOT reach `0x50330`, the `0xFE` byte seen there during the
live MCP session is from CHECKIT's own runtime (CHECKIT.CNF data etc.).

---

## Log Comparison Analysis (oxide86.log vs oxide86-working-hdd.log)

`python3 scripts/compare_logs.py oxide86.log oxide86-working-hdd.log`

Found **26 divergences**:
- Divergences 1–25: minor log-format differences only — `F3` (REP prefix) is
  logged as a separate instruction in one emulator and combined with the following
  byte in the other. Both logs execute the same code; these are not real behavioral
  differences.
- **Divergence 26** (permanent — no resync): at `oxide86.log` line 2741 /
  `oxide86-working-hdd.log` line 2665. Both are at address `0070:23BB`.

### INT 1Ah AH=02h divergence

DOS boot code at `0070:23AE`–`0070:23CB`:

```
xor cx, cx          ; CX = 0
xor dx, dx          ; DX = 0
mov ah, 0x02
int 0x1a            ; AH=02h: read RTC time
cmp cx, 0x0000
jne 0x23cd          ; ← jumps in working emulator, falls through in ours
cmp dx, 0x0000
jne 0x23cd
cmp bp, 0x0001      ; retry counter
je  0x23e1          ; give up if already retried
inc bp
mov cx, 0x4000
loop 0x23c9         ; delay, then retry INT 1Ah
jmp 0x23ae
```

- **Our emulator (8086 CPU):** `has_rtc()` = false (no RTC for 8086 mode) →
  INT 1Ah AH=02h sets CF=1, leaves CX=DX=0. Both `jne` branches fall through.
  After one retry still gets zeros; BP=1 → `je 0x23e1` gives up. The byte at
  `cs:[0x04F3]` (physical `0x0BF3`) is **never** written with `0x01`.
- **Working emulator (also 8086 CPU):** returns CX≠0 from INT 1Ah AH=02h.
  `jne 0x23cd` is taken. Writes `0x01` to `cs:[0x04F3]` and continues normal
  init.

This is non-standard behavior by the working emulator: on a real 8086/XT with
no RTC, INT 1Ah AH=02h would return CF=1 without touching CX/DX, giving the
same CX=DX=0 result. The working emulator appears to return BDA timer ticks
in CX:DX (non-zero after a few seconds of boot) even in 8086 mode.

The byte at `cs:[0x04F3]` is a DOS internal RTC-valid flag. Its absence may
affect how DOS initialises the file system or memory arena, but the impact on
CHECKIT's load address and overlay buffer placement is **not yet determined**.

### In our 286 mode

With `--cpu 286` (the default), the RTC is created and returns real system
time. INT 1Ah AH=02h returns non-zero CX (e.g. `0x0816` for 8:22 AM), the
`jne 0x23cd` is taken, and DOS takes the same path as the working emulator.
Despite this, CHECKIT still crashes at `91C2:4D02` — the buffer overlap writes
`0x64` to `[DS:031E]` regardless of CPU mode, because CHECKIT's load address
and the Borland OVR buffer address are identical in both 286 and 8086 modes.

The INT 1Ah divergence is therefore a **separate bug** (our emulator returns
the wrong values for 8086 + no RTC) but it is **not the root cause** of the
`91C2:4D02` crash.

---

## Open Questions

1. ~~**What is CX for the overlay AH=3F call?**~~ **RESOLVED:** CX=65520 for
   first two reads, 15664 for the third. The second read (`0x39F80`–`0x49F6F`)
   writes `0x64` to `[DS:031E]`. The overlays do not reach `0x50330`.

2. ~~**Why does CHECKIT only get `0x40EF` paragraphs (≈260 KB)?**~~ **RESOLVED:**
   COMMAND.COM does NOT call AH=4A before EXEC. CHECKIT receives the **full**
   available free block (`0x90BC` paragraphs, PSP:0002=`0xA000`). CHECKIT's own
   Borland overlay manager immediately shrinks itself: at `19CD:000E`–`19CD:0064`
   (log ~line 854661) it reads `PSP:0002 = 0xA000`, computes
   `new_top = DS(0x4033) + 0x1000 = 0x5033`, writes `0x5033` back to PSP:0002,
   then calls `AH=4A` (BX=`0x40EF`, ES=PSP) to release the memory above `0x5033`.
   The MCB at `0x5033` with size `0x4FCC` is the resulting **free block** — it is
   not COMMAND.COM's transient. COMMAND.COM's resident block ends near `0x0E10`,
   and its AUTOEXEC buffer is at `0x9FFAF` — both far from `0x5033`.

3. ~~**What is at segment `0x5033` before CHECKIT corrupts it?**~~ **RESOLVED:**
   Free DOS memory, not COMMAND.COM's transient. See Q2 above. CHECKIT freely
   writes beyond `0x5032F` because it treats the freed-back memory as its arena.

4. **286 vs 8086 behavioral difference in CHECKIT.**
   Memory layout is identical in both modes. The observed difference is almost
   certainly **instruction compatibility**: CHECKIT.EXE was compiled for 286 and
   uses 286-specific opcodes (C0/C1 shift-by-imm, 0x68 PUSH imm, PUSHA/POPA,
   ENTER/LEAVE, etc.). On 8086 mode these trigger INT 6 (invalid opcode). Our
   BIOS INT 6 handler is a no-op (just IRET), so control returns to MS-DOS's own
   INT 6 handler, which terminates the program. On 286 mode those instructions
   execute normally and CHECKIT reaches the `91C2:4D02` crash point.

5. **Should `[4033:031E]` be `0x00` before the overlay runs?**
   Yes — this is BSS (102,192 bytes past the EXE image). If the emulator doesn't
   zero BSS on `AH=4B` EXEC, prior memory contents remain. However, the `0x64`
   is demonstrably written by the second overlay AH=3F read, not by stale BSS.

6. **Borland Overlay Relocation**
   The relocation pass processes overlay buffers and adds PSP (`0x0F54` or
   similar) to stored relative segment values. DS = `0x4033` is CHECKIT's
   permanent global data segment — not itself a relocation target.

7. **Why does running from floppy work but from HDD fail?**
   Confirmed: both `checkit-floppy.exe` and `checkit-hdd.exe` (which differ by
   only 41 bytes in overlay string data, neither at DS:031E) work from floppy
   A: but fail when run from HDD C:. The disk read paths (floppy INT 13h and
   HDD ATA) were audited and are both correct — the data read from the file is
   identical in both cases.

   The most likely explanation is **hardware detection**: when CHECKIT is run
   from floppy without an HDD attached, it detects no hard disk (INT 13h AH=15h
   or AH=08h returns "not present" for drive 0x80) and skips the HDD diagnostic
   overlays entirely. The crashing overlay module is only loaded when CHECKIT
   decides to run HDD tests.

   **Verification test:** run the emulator with both `--hdd tmp/hdd.img` AND
   `--floppy-a disk.img`, then run `a:checkit` (from floppy). If it crashes,
   this confirms the crash is triggered by HDD detection, not by where the EXE
   resides.

8. **Why does the working emulator (separate codebase) not have the buffer overlap?**
   Unknown. Possible explanations: (a) the Borland OVR buffer lands at a different
   physical address (different DOS kernel size, different PSP allocation), (b) the
   working emulator's CHECKIT.EXE is a different build (8086-compiled, smaller BSS,
   different overlay geometry), or (c) the working emulator's EXEC implementation
   allocates memory differently so the buffer does not overlap DS. The INT 1Ah
   divergence could indirectly affect the load address if the RTC-valid flag at
   `cs:[0x04F3]` changes how DOS sizes the transient area — but this has not been
   confirmed.

---

## Debugging Tooling / Suggested Code Changes

### 1. Log CX in `log_int21_dos_call` for AH=3F

**Problem:** We don't know the requested byte count passed to AH=3F.

**Fix:** In `log_int21_dos_call` (int21_dos_services.rs), save `cx` in
`PendingDosRead` and log it:

```rust
// Add to PendingDosRead struct:
pub(in crate::cpu) cx: u16,   // requested byte count

// In the AH=3F branch:
self.pending_dos_read = Some(PendingDosRead {
    ...
    cx,
    ...
});
```

```rust
// In check_pending_dos_read, change the log line to include cx:
log::debug!(
    "[DOS] AH=3F read \"{filename}\" pos={file_pos} cx={} → phys 0x{base:05X}-0x{phys_end:05X} ({bytes_read} bytes)",
    pdr.cx
);
```

---

### 2. Fix `check_pending_dos_read` — snapshot before the read

**Problem:** The current implementation reads the memory value *after* the DOS
call returns, so it can't distinguish "DOS read wrote 0x64" from "the value was
already 0x64 before the read".

**Fix:** When `PendingDosRead` is created (at the INT 21h trap), snapshot the
current value at every watched address that falls in the buffer range. On
return, compare old vs new value.

```rust
// In PendingDosRead struct — add:
pub(in crate::cpu) pre_read_values: Vec<(usize, u8)>, // (phys_addr, old_val)

// In log_int21_dos_call when saving PendingDosRead:
let base = bus.physical_address(ds, dx);
let len  = cx as usize;          // CX = requested byte count
let pre_read_values = bus.watchpoints_in_range(base, len)
    .iter()
    .map(|&addr| (addr, bus.memory_read_u8(addr)))
    .collect();

// In check_pending_dos_read, replace the current watchpoint loop:
for (addr, old_val) in &pdr.pre_read_values {
    let new_val = bus.memory_read_u8(*addr);
    let offset  = addr - base;
    log::info!(
        "[WATCH] 0x{addr:05X}: 0x{old_val:02X} → 0x{new_val:02X} by DOS AH=3F \"{filename}\" pos={}",
        file_pos + offset as u32,
    );
}
```

---

### 3. Fix BIOS path to call set_current_ip

**Problem:** The BIOS early-return path in `cpu/mod.rs` returns before calling
`bus.set_current_ip(pre_cs, pre_ip)`, so watchpoint hits during BIOS execution
report the last non-BIOS instruction's address.

**Fix:** Call `set_current_ip` at the top of the BIOS branch too:

```rust
if self.cs == BIOS_CODE_SEGMENT {
    bus.set_current_ip(self.cs, self.ip);   // ← add this
    self.step_bios_int(bus, self.ip as u8);
    ...
}
```

---

### 4. Add watchpoint coverage to `bus.load_at`

**Problem:** `bus.load_at` → `memory.load_at` → `copy_from_slice` bypasses
`memory_write_u8` entirely.

**Fix:** After `self.memory.load_at(addr, data)` in `bus.rs`, scan for watched
addresses in the written range:

```rust
pub fn load_at(&mut self, addr: usize, data: &[u8]) {
    self.memory.load_at(addr, data);
    for &wp in &self.watchpoints {
        if wp >= addr && wp < addr + data.len() {
            let val = data[wp - addr];
            log::info!("[WATCH] 0x{wp:05X} written: 0x{val:02X} by bus.load_at base=0x{addr:05X}");
        }
    }
}
```

---

### 5. Verify INT 13h buffer addresses match AH=3F buffer

**Problem:** The INT 13h ES:BX is the authoritative destination for disk-sector
data. Cross-checking it against the `AH=3F` DS:DX buffer would confirm the data
actually lands where expected.

```rust
log::debug!(
    "[INT13] AH=02 read {} sectors → ES:BX = {:04X}:{:04X} (phys 0x{:05X})",
    al, es, bx, bus.physical_address(es, bx)
);
```

---

## Summary

The crash is caused by the init flag `[4033:031E]` (physical `0x4064E`) being
`0x64` when the 91C2 overlay runs its first-time init check. Because the flag is
non-zero, the init code that would populate `[CS:8D77]` is skipped, a garbage DS
value is loaded, and the subsequent far call crashes.

The `0x64` is written by the overlay load itself: the Borland overlay manager reads
65520 bytes from disk into a buffer at segment `39F8` (physical `0x39F80`). That
buffer physically overlaps DS=`4033` (physical `0x40330`), and the byte at file
offset 301950 — the `'d'` in `"d ram_basebeg\t0x..."` — lands exactly at `[DS:031E]`.

DS is in BSS (102192 bytes past the EXE image) and should be zero on program load.
The `0x64` is not supposed to be there; either the emulator doesn't zero BSS pages,
or the AH=3F byte count (CX) is larger than it should be on real DOS, or the buffer
is placed at an address that shouldn't overlap DS.

**Most actionable next step:** log CX for AH=3F calls and compare against the
`0x66CE` (26318-byte) threshold. If CX ≥ 26318 in the emulator but < 26318 on
real DOS, that's the direct cause.
