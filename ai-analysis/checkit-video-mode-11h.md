# CheckIt Video Grid Test #9 - Mode 11h Analysis

Analysis of `oxide86.log` from a DOS video diagnostic program running "Graphics Grid Test #9 of 10".
This test is supposed to display a **mode 11h** (640x480 monochrome) grid test: black background,
white double-line border, white dotted grid lines in the middle.

## Full Mode Switch Timeline

| Line | Time | Event |
|------|------|-------|
| 479 | 01:01.933 | INT 10h/AH=00h: set video mode (previous test teardown) |
| 2481 | 01:01.970 | `set mode: 0x03` — clear screen for menu |
| 2702 | 01:01.976 | INT 10h/AH=00h: set mode again (after 400-line select) |
| 4704 | 01:02.013 | `set mode: 0x03` — second mode 03h set |
| 27551 | 01:02.726 | INT 10h/AH=00h: set video mode (begin test #9 init) |
| 29553 | 01:02.755 | `set mode: 0x03` — first reset for test #9 |
| 29771 | 01:02.760 | INT 10h/AH=00h: set mode again (after 400-line select) |
| 31773 | 01:02.789 | `set mode: 0x03` — second reset |
| **32452** | **01:02.806** | **`set mode: 0x11`** — **the actual test mode** |
| 32453 | 01:02.838 | `set mode: 0x11` (logged twice) |
| 42779 | 01:03.109 | GUI resizes pixel buffer to 640x480 |

The program sets mode 03h **four times** before finally switching to mode 0x11. The mode 03h
sets are:
1. **Previous test teardown** — restoring text mode from whatever the prior test used
2. **After 400-line select** — INT 10h/AH=12h/BL=30h selects 400 scan lines, then re-sets mode 03h to apply it
3-4. **Test #9 init** — same pattern repeated: set 03h, configure 400 lines, set 03h again

Then mode 0x11 is set for the actual graphics test.

## What Happens in Mode 11h

### Mode set and BDA update (line 32452)
- Mode 0x11 is set: 640x480 monochrome VGA graphics
- BDA updated: cols=80, rows=30, page_size=4800
- Pixel buffer resizes to 640x480 (but not until line 42779 — ~300ms later)

### Grid drawing gap (lines 32453–36286)
Between the mode 11h set and the first INT 10h text write, there are ~4000 lines of execution.
This is where the **dotted grid should be drawn**, but:
- **No INT 10h/AH=0Ch** (write pixel) calls exist anywhere in the log
- **No direct VRAM writes to 0xA0000** are logged
- The code appears to be doing **printf/string formatting** (building the title string "Mode 03h"),
  data structure setup, and lookup table operations

**The grid is NOT being drawn.** The program likely uses direct VGA framebuffer writes
(segment A000h, writing to physical 0xA0000+) which aren't visible in the execution log since
port IO and memory-mapped writes to the VGA range aren't traced at the instruction level.

### Border and text overlay (lines 36286–90133)
After the (missing) grid drawing, the program draws a text overlay using BIOS calls while
**still in mode 0x11** (there is no mode switch back to 03h):

- **INT 10h/AH=09h** writes 728 characters one at a time, all with attr 0x07 (white on black)
- **INT 10h/AH=02h** positions the cursor before each character
- The overlay includes:
  - **Row 0**: Double-line box top (`╔═══...═══╗`) with title
  - **Rows 1–23**: `║` left/right borders
  - **Row 24**: Double-line box bottom (`╚═══...═══╝`)
  - **Row 20**: "The dotted lines in the grid should be straight and uniform."
  - **Row 22**: "Is the screen display correct?" with Y selection highlight
  - **Row 23**: "Y-Yes  N-No  ESC-Interrupt"

**Problem: The title says "Mode 03h" but should say "Mode 11h".** The title text is
generated from data that contains "03h" rather than "11h" — this may be a bug in the
test program's display string, or the program may be reading the mode number from a
variable that was set during the mode 03h initialization phase and not updated after
switching to 0x11.

### INT 11h calls

**First call** (line 5176, ~01:02.026) — during initial mode 03h setup:
- Returns AX=0x0063: bits 4-5 = `10` (80-column CGA)
- Program stores AL=0x63 at [0xADEE], uses bits 4-5 to pick attr 0x07

**Second call** (line 90392, ~01:04.495) — after all drawing is complete:
- Same return AX=0x0063
- Used to determine cursor shape, then INT 10h/AH=01h restores visible cursor (lines 12-15)

Both INT 11h calls use a generic interrupt dispatch wrapper at `36C5:099A` which:
1. Builds a `CD xx` / `CB` thunk on the stack
2. Loads registers from a parameter block
3. Calls the thunk via far call
4. Saves all returned registers back to the parameter block

The program then extracts `(AX & 0x30) >> 4` to classify the adapter:
- 0 = reserved
- 1 = 40-col CGA
- 2 = 80-col CGA → selects attr 0x07
- 3 = monochrome

## Expected vs Actual Display

**Expected:** Black screen with white double-line border and white dotted grid lines filling
the interior of the border. 640x480 monochrome (mode 11h).

**Actual (likely):** The mode 11h framebuffer may be blank (black) since the grid drawing
uses direct VRAM writes that we can't verify from the log. The text overlay (border + prompt)
is drawn via BIOS INT 10h calls and should be visible if INT 10h/AH=09h works correctly in
mode 11h.

## Root Cause: Why the Title Shows "Mode 03h"

### The mode validation check

The program's mode-set wrapper at `20D4:0150` (approximate) calls INT 10h/AH=00h to set the
video mode, then **verifies** the mode was set correctly by comparing against a stored mode
variable at `[457C:0358]`:

```
20D4:0195  mov ax, [bp+0x06]        ; AX = requested mode (0x0011)
20D4:0198  cmp [0x0358], ax         ; [457C:0358] = 0x0003 (previous mode)
20D4:019D  jne 0x01a3               ; modes differ → FAILURE
20D4:01A3  mov ax, 0xffff           ; return -1
```

The comparison at `20D4:0198` checks if `[0x0358]` (the wrapper's internal mode tracking
variable) matches the requested mode. **It still reads 0x0003** (the mode from the previous
test), meaning the variable was not updated after the INT 10h call returned. The wrapper
returns -1 (failure) without updating `[0x0358]` to 0x11.

### The sprintf reads the stale mode

Later, the title is built via sprintf at `1E88:0D8F`:

```
1E88:0D8F  push [0x0358]    ; pushes 0x0003 (stale!) as the mode arg
1E88:0D94  mov ax, 0x9099   ; test name string
1E88:0D97  push ds
1E88:0D98  push ax
...
1E88:0DA3  call 0x1eaa      ; calls the title-drawing function
```

The format string at `48A2:932E` is:
`"Graphics Grid Test #%d of %d (Mode %02Xh)"`

The `%02X` reads the mode value from the stack = **0x0003**, producing "03h".

### Why `[0x0358]` isn't updated

The wrapper's logic appears to be:
1. Call INT 10h/AH=00h to set mode
2. After return, compare `[0x0358]` against the requested mode
3. If they match → return success, mode is already tracked
4. If they differ → return -1 (failure), do NOT update `[0x0358]`

The problem is that `[0x0358]` should be updated **before** or **during** the mode set,
not compared afterwards as a validation gate. On real hardware (or DOSBox), `[0x0358]`
would presumably be updated by the successful INT 10h call itself (perhaps the wrapper
updates it before the comparison in a code path we're missing, or the BIOS updates it
via a different mechanism).

### Root cause: INT 10h/AH=1Bh returns null static functionality table

The BDA code is correct — `bda_set_video_mode(bus, mode.as_u8())` writes 0x11 to the
BDA and `Mode::M11Vga640x480x2.as_u8()` returns 0x11 correctly. The emulator IS running
in VGA mode (`--video-card vga`).

The failing capability check at `1A34:14BB`:

```
1A34:14BE  test [0xcfc5], 0x10     ; 0x18 & 0x10 = 0x10 → VGA detected ✓
1A34:14C5  test [0xcfc0], 0x02     ; 0xC5 & 0x02 = 0   → mode 11h NOT supported ✗
1A34:14CA  je 0x150b               ; taken → "stc; ret" (not supported)
```

`[0xcfc5]` bit 4 (VGA presence) is correctly set (= 0x18). But `[0xcfc0]` bit 1
(mode 0x11 support) is clear (= 0xC5 = 11000101).

The capability flags `[0xcfc0]` are built during CheckIt initialization (before exec
logging) by calling **INT 10h/AH=1Bh** (Get Functionality/State Information). The
response at offset 00h-03h contains a pointer to the **static functionality table**
which describes supported video modes.

In `int10_video_services.rs:1974`:
```rust
// Offset 00h-03h: Pointer to static functionality table
bus.memory_write_u16(buffer_addr, 0x0000);     // NULL!
bus.memory_write_u16(buffer_addr + 2, 0x0000); // NULL!
```

**The static functionality table pointer is 0000:0000 (null).** This table is supposed
to contain a bitmask of supported video modes. Byte 2 of the table covers modes 0x10-0x17,
and bit 1 of byte 2 = mode 0x11. Since the pointer is null, CheckIt cannot determine
that mode 0x11 is supported and defaults to "not supported."

**Fix:** Implement the static functionality table in ROM BIOS memory and point to it
from the AH=1Bh response. The table needs at minimum the supported video modes
bitmask with mode 0x11's bit set (byte 2, bit 1).

## Other Issues to Investigate

1. **Missing grid content** — The actual pixel grid is drawn via direct writes to VGA memory
   (segment A000h). Need to verify these writes are hitting the framebuffer correctly.
   Could be a VGA write mode / plane mask issue since mode 11h uses planar graphics.

2. **INT 10h/AH=09h in graphics mode** — Writing characters in mode 11h requires the BIOS
   to render font glyphs as pixel patterns into the framebuffer, not simple char+attr pairs
   to text VRAM. Need to verify this path works correctly.
