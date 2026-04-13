# CD-ROM `dir d:` — "CDR101: Not Ready" Root Cause Analysis

## Problem

Running `dir d:` on the CD-ROM drive produces "CDR101: Not ready reading drive D" in the oxide86 emulator. This document records the full call-chain analysis from MSCDEX down to the SBPCD.SYS hardware interface.

## Trace file

`asm-analysis/cd-rom-dir.asm` — execution log of a `dir d:` attempt with a disc present.  
Config: `asm-analysis/cd-rom-dir.json`.

## Confirmed failure path (from trace)

```
MSCDEX (seg 0019)
  func_wait_for_data (0019:51A6)
    → polls func_poll_drive 84 times
    → 84th poll returns ZF=0 / AL=0x0D → "not ready"
  → func_dispatch_error (0019:5036) with cmd 0x06 (Read Long)
    → patches return addr to 0019:92E7
  → INT 24h at 0019:9337 → "CDR101: Not ready reading drive D"

SBPCD.SYS interrupt handler (seg 0C45)
  0C45:0C4F  in al, dx           ; read base+1 → 0x00 (no ATN)
  0C45:0C52  test al, 0x01       ; ZF=1 → no ATN
  0C45:0C54  je 0x0C6D           ; taken → skip ATN handler
  0C45:0C6D  mov [0x0054], 0x01
  0C45:0C73  test [0x0053], 0x04 ; [0x0053]=0x48 & 0x04 = 0 → jne not taken
  0C45:0C87  jmp 0x0CF2
  0C45:0CF2  test [0x0053], 0x01 ; [0x0053]=0x48 & 0x01 = 0 → ZF=1
  0C45:0CF8  je 0x0D23           ; taken → skip geometry init
  0C45:0D23  test [0x002e], 0x04 ; [0x002e]=0x00 → ZF=1 → jne not taken
  0C45:0D2B  test [0x0053], 0x01 ; [0x0053]=0x48 & 0x01 = 0 → ZF=1
  0C45:0D31  je 0x0D9A           ; taken → dispatch anyway
  0C45:0D9E  jmp [cs:si]         ; dispatches command handler
  ...command handler runs...
  0C45:199F  test [0x002e], 0x04 ; [0x002e]=0x00 → STILL zero → ZF=1
  0C45:19A5  je 0x19BF           ; taken → error path
  0C45:19BF  jmp 0x0DAE          ; → mov al,0x02; mov ah,0x81 → error status
  0C45:0DE1  mov [si+0x03], ax   ; writes 0x8102 to request block
```

The request block status 0x8102: bit 1 of the high byte (0x81) is **clear** → MSCDEX at `0019:8458` reads this, `and ah, 0x02` → ZF=0 → "not ready".

## Key variable: `[CS:0x002E]` — drive-init flags

Located at physical 0xC47E. SBPCD.SYS uses this as a bitmask of completed initializations:

| Bit | Mask | Set by | Meaning |
|-----|------|--------|---------|
| 4   | 0x10 | `0x1AF2`: `or [cs:0x2e], 0x10` | cmd 0x8B (disc position) read successfully |
| 2   | 0x04 | `0x0D94`: `or [cs:0x2e], 0x04` | cmd 0x88 (disc geometry) read successfully |

**Both bits are 0x00 in the trace** — neither initialization step ran.

## Why initialization never ran: `[CS:0x0053]` bit 0

`[CS:0x0053]` holds the last result byte from hardware cmd 0x81 (Get Attention Status).

The geometry init at 0x0D33 is only reached when `[0x0053] & 0x01 != 0` (bit 0 set). In the trace, cmd 0x81 returns **0x48**, which has bit 0 clear. So the `je 0x0D23` at 0x0CF8 is always taken, bypassing both init paths.

```
[0x0053] = 0x48 = 0b01001000
  bit 6 (0x40): disc stable / not changed ✓
  bit 3 (0x08): drive ready / data disc   ✓
  bit 0 (0x01): status-changed/ATN event  ✗ ← not set → init never runs
```

## Why ATN (bit 0) is never seen

The emulator's `attention_pending` flag fires ATN (returns 0x01 from base+1 in Idle state) correctly. However, `call 0x218E` (the ATN-path helper) **re-issues cmd 0x81** and reads a fresh result into `[0x0053]`. If the fresh result is 0x48 (no bit 0), the init still won't run even after ATN fires.

### The fix

**Cmd 0x81 must return bit 0 set (0x49) when a disc-change/ATN event is pending.**

In real Panasonic hardware, bit 0 of the cmd 0x81 response means "status has changed since last check" (disc inserted, door changed, etc.). It is asserted once per ATN event and cleared on subsequent polls. SBPCD uses it as a trigger to re-read disc geometry.

Implementation in `sound_blaster_cdrom.rs`: add a `status_changed` flag (set alongside `attention_pending` on disc load, cleared after the first cmd 0x81 that returns bit 0). When `status_changed` is true, cmd 0x81 response = `0x48 | 0x01 = 0x49`.

## Two additional commands to implement

For the geometry init paths to complete, SBPCD must successfully execute two commands it has never been sent before:

### cmd 0x8B — Read Disc Position/Mode (6 result bytes)

Called from `func_sbpcd_read_disc_pos` (0x1AAE) when `[0x002e]` bit 4 is clear.  
Result buffer at `[CS:0x9D]`:

| Byte | Field | Usage |
|------|-------|-------|
| 0    | mode  | stored to `[CS:0x0075]`; if 0x20 → different sector CX in later read |
| 1    | pos_M | stored to `[CS:0x003F]` |
| 2    | pos_S | stored to `[CS:0x0040]` |
| 3    | pos_M2 | stored (AH part) to `[CS:0x0041]` |
| 4    | pos_S2 | stored (AL part) to `[CS:0x0041]` |
| 5    | pos_F  | stored (DL part) to `[CS:0x0043]` |

On success: sets `[0x002e]` bit 4 (0x10) via `or byte [cs:0x2e], 0x10` at 0x1AF2.  
Safe stub response: 6 zero bytes `[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]`.

### cmd 0x88 — Read Disc Geometry (5 result bytes)

Called from the 0x0D33 path with CX=5.  
Result buffer at `[CS:0x9D]`:

| Byte | Field | Usage |
|------|-------|-------|
| 0    | geo_hi | high byte of 24-bit geometry value → DL |
| 1    | geo_mid | mid byte → AH |
| 2    | geo_lo | low byte → AL |
| 3    | unit_hi | high byte of sector-unit size → BH |
| 4    | unit_lo | low byte of sector-unit size → BL |

The code at 0x0D61–0x0D93 computes:

```
if BX == 0 → error
if BX > 2048 → cap to 2048
if 2048 % BX != 0 → error
geometry = (2048 / BX) * 151 + bytes[0:2] as 24-bit
→ stored to [CS:0x3B:0x3D]
or byte [cs:0x2e], 0x04   ← sets the drive-ready flag
```

**Bytes 3-4 must be non-zero and must divide 2048 evenly** (power of 2 ≤ 2048).  
Safe stub: `[0x00, 0x00, 0x00, 0x08, 0x00]` → BX=0x0800=2048, `2048/2048=1`, geometry=151.

## Full initialization sequence (once fixed)

On first SBPCD interrupt handler call during `dir d:`:

1. Read base+1 → **0x01** (ATN asserted, `attention_pending=true`)
2. ATN path at 0x0C56: call `func_sbpcd_get_attn_status` (0x218E)
   - Sends cmd 0x81 → result **0x49** (`status_changed=true` → bit 0 set)
   - Stores `[0x0053] = 0x49`; clears `status_changed`
3. Result bit 6 set → check bit 4 → clear → fall to 0x0C6D
4. `test [0x0053], 0x01` → bit 0 set → `je 0x0D23` NOT taken → fall to 0x0CFA
5. `test [0x002e], 0x10` → bit 4 clear → call `func_sbpcd_read_disc_pos` (0x1AAE)
   - Sends cmd **0x8B** → reads 6 bytes → **`or [0x002e], 0x10`**
6. Jump to 0x0D23: `test [0x002e], 0x04` → bit 2 still clear
7. `test [0x0053], 0x01` → bit 0 set → NOT taken → fall to 0x0D33
8. Sends cmd **0x88** → reads 5 bytes → **`or [0x002e], 0x04`** at 0x0D94
9. Dispatch at 0x0D9A; command handler runs; 0x199F check passes (bit 2 set) ✓

On subsequent calls: `[0x002e]` = 0x14 (bits 2+4 set), both checks pass immediately.

## Summary of required emulator changes

| Change | File | Detail |
|--------|------|--------|
| Add `status_changed` flag | `sound_blaster_cdrom.rs` | Set `true` in `new()` and `load_disc()`; cleared after first cmd 0x81 when true; cmd 0x81 returns `0x49` when set, `0x48` otherwise |
| Implement cmd 0x8B | `sound_blaster_cdrom.rs` | Returns 6 bytes; safe stub: `[0x00; 6]` |
| Implement cmd 0x88 | `sound_blaster_cdrom.rs` | Returns 5 bytes; bytes 3-4 must be non-zero and divide 2048; safe stub: `[0x00, 0x00, 0x00, 0x08, 0x00]` |
