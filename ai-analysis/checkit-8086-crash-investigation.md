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
| Global data segment | `DS = 4033` (throughout execution) |
| Physical DS base | `0x40330` |
| Failing step | Step 7 of 12 |

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

## Physical Address `0x4064E` — Timeline of Writes

| Log line | Event | Value written |
|----------|-------|---------------|
| 131634 | `[WATCH]` — written by `0070:079C` | `0x64` |
| 132422 | DOS `AH=3F` read starts: `pos=275632`, buffer `0x39F80–0x49F6F` (65520 bytes) | — |
| 132424 | `check_pending_dos_read` reports `0x4064E` in buffer at offset `0x66CE`, file pos `301950`, value `0x64` | `0x64` (current value) |

**Key observation:** the buffer `0x39F80–0x49F6F` contains `0x4064E` (offset
`0x66CE`), yet **no `[WATCH]` entry fires during the actual write**. The value
`0x64` reported at line 132424 is the one left by the `0070:079C` write at
line 131634.

---

## The Watchpoint Gap

The `check_pending_dos_read` implementation reads the **current** memory value
when the `AH=3F` call returns, not what the DOS read itself wrote. This means:

- If the DOS read wrote `0x00`, the current value would be `0x00` and we'd see
  that — but only if no later write put `0x64` back.
- If the DOS read wrote `0x64`, no observable difference from the prior
  `0070:079C` write.
- If the DOS read used a path that **bypasses `bus.memory_write_u8`**, no
  `[WATCH]` entry fires at all — which is exactly what we observe.

### Known bypass path

`bus.load_at` → `memory.load_at` → `copy_from_slice` completely bypasses
`memory_write_u8` and therefore bypasses all watchpoints. This path is used
for boot sector loading and initial program loading; it is **not** supposed to
be used for DOS file reads.

### Checked — no gap found there

- `bus.memory_write_u16` / `bus.memory_write_u32` → both call `memory_write_u8` ✓
- REP MOVSB / MOVSW in `string.rs` → both call `bus.memory_write_u8` ✓
- All `self.memory.*` call-sites in `bus.rs` — only `memory.write_u8` (has
  watchpoints) and `memory.load_at` (no watchpoints, not used for DOS reads) ✓

---

## File Content at Offset 301950

```
$ xxd -s 301950 target/CHECKIT.EXE | head -2
000499be  64 20 72 61 6d 5f 62 61  73 65 62 65 67 09 30 78  |d ram_basebeg.0x|
```

Byte `0x64` = `'d'`, the first character of the format string
`"d ram_basebeg\t0x%07lx\n"` in the overlay's data section. This is what gets
written to `[4033:031E]` by the overlay load — confirming the init flag
contains data, not a zeroed flag, after loading.

---

## Borland Overlay Relocation

At log line 223892, the relocation pass processes the third overlay buffer:

```
add [0x089c], dx   ; ES=4BA0, value 0x4033 → 0x4F87
```

Physical address `0x4C29C` (inside the overlay buffer). The overlay manager
adds PSP (`0x0F54`) to stored relative segment values. DS = `4033` is
CHECKIT's permanent global data segment (set at `19CD:137C/13B3` via
`push ds` / `pop ds`), not the relocated value.

---

## Open Questions

1. **Is there a remaining watchpoint gap in the DOS `AH=3F` path?**
   The writes to `0x39F80–0x49F6F` do not trigger the watchpoint, even though
   `0x4064E` falls in that range. Either:
   - The actual write path uses `bus.load_at` somewhere, OR
   - The saved DS:DX in `PendingDosRead` resolves to a different physical range
     than where the data lands (possible if DS changes between INT 21h trap and
     return).

2. **Does the DOS read write `0x64` or `0x00` to `0x4064E`?**
   The correct answer determines whether:
   - The file's content is the root cause (offset 301950 = `0x64` always sets
     the flag), OR
   - Some post-read code re-writes `0x64` back (e.g. the `0070:079C` write
     survives because the DOS read writes to a different range).

3. **What is `0070:079C`?**
   Physical `0x0E9C` — this is in the low DOS data area. The write at line
   131634 comes from here, but the context is not yet confirmed (suspected INT
   13h or RAM test routine in early DOS init).

4. **Unrecognised opcodes at `19CD:1382` and `19CD:1396` (`db 0x8f`)**
   These may affect program flow before the init check runs.

---

## Debugging Tooling / Code Changes

### 1. Fix `check_pending_dos_read` — snapshot before the read

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

This tells us definitively whether the DOS read changed the value.

---

### 2. Add watchpoint coverage to `bus.load_at`

**Problem:** `bus.load_at` → `memory.load_at` → `copy_from_slice` bypasses
`memory_write_u8` entirely. Any overlay data loaded this way is invisible to
the watchpoint system.

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

### 3. Log the disassembled instruction at every watchpoint hit

**Problem:** `[WATCH] 0x4064E written: 0x64 by 0070:079C` tells us the CS:IP
but not *what instruction* ran. For addresses like `0070:079C` in the DOS data
area it's hard to tell whether this is code or a stray write.

**Fix:** In `bus.memory_write_u8`, when a watchpoint fires, also log a few
bytes of the instruction stream so the cause is self-documenting in the log.
Alternatively, the CPU's existing instruction trace (if enabled) can be cross-
referenced with the CS:IP — check `oxide86.log` for lines near 131634 with
CS=0070 IP=079C.

---

### 4. Confirm what `0070:079C` is

The write at log line 131634 comes from physical `0x0E9C` (segment `0070`,
offset `079C`). This is in the real-mode IVT/BDA/DOS data region.

**Steps to identify it:**
1. Enable instruction-level trace (`log::trace!` in the fetch/decode loop or
   use the existing `--trace` flag if present).
2. Search the log for `CS=0070 IP=079C` just before line 131634.
3. Alternatively, add a **read-only snapshot** of the bytes at `0x0E9C` in
   `bus.rs` to see if that address contains code or data at boot time.

---

### 5. Verify the `PendingDosRead` buffer range is correct

**Problem:** `PendingDosRead` saves DS:DX at the time of the INT 21h trap. If
the actual DOS `AH=3F` handler modifies the buffer pointer before writing (some
DOS kernels do pointer fixups), the saved address may not match where the bytes
actually land.

**Fix:** Cross-check the `phys` range printed by `check_pending_dos_read`
(`0x39F80–0x49F6F`) against the INT 13h calls that happen *inside* the DOS
read. The INT 13h ES:BX buffer address is the authoritative destination for
disk-sector data. Add logging to `handle_int13_disk_services`:

```rust
log::debug!(
    "[INT13] AH=02 read {} sectors → ES:BX = {:04X}:{:04X} (phys 0x{:05X})",
    al, es, bx, bus.physical_address(es, bx)
);
```

Compare those physical addresses with `0x39F80–0x49F6F` to verify the overlay
data actually lands in the expected range.

---

### 6. Add a "first write only" watchpoint variant

For tracking the init flag specifically: the current watchpoint fires on
*every* write, including writes of the same value. A variant that only fires
when the value *changes* would reduce noise and make it easier to see the
transition that matters (e.g. `0x64` → `0x00` if the overlay load does zero
the flag).

This could be a separate `Vec<usize>` — `change_watchpoints` — checked in
`memory_write_u8` as:

```rust
for &wp in &self.change_watchpoints {
    if addr == wp {
        let old = self.memory.read_u8(addr);  // read BEFORE the write
        if old != val {
            log::info!("[WATCH-CHANGE] 0x{wp:05X}: 0x{old:02X} → 0x{val:02X}");
        }
    }
}
```

---

## Summary

The crash is caused by the init flag `[4033:031E]` (physical `0x4064E`) being
non-zero (`0x64`) when the 91C2 overlay runs its first-time check. Because the
flag is non-zero, the actual init code that would set up `[CS:8D77]` is
skipped. Later code loads DS from the garbage value at `[CS:8D77]` and crashes.

The `0x64` value comes from the overlay file itself (file offset 301950 is
`'d'`). A write from `0070:079C` also puts `0x64` there before the overlay
load. The watchpoint implementation in `check_pending_dos_read` reads the
current memory value on return rather than intercepting the actual write,
making it impossible to distinguish which write is "last" from the logs alone.

The most likely fix direction: the init flag should be zeroed before the 91C2
overlay module runs. Either the emulator needs to correctly simulate whatever
DOS/CHECKIT does to zero that region, or there is a bug in the overlay load
that puts file data where the flag should be.
