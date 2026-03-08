# Alley Cat (CAT.EXE) - Cat Movement Investigation

## Problem Statement

Pressing the right arrow key does not cause the cat to move visibly. Investigation traced
the full code path from keyboard input through to the movement/scroll mechanism.

---

## Runtime Memory Layout

- CS = 0x160F (code segment)
- DS = 0x0EFC (data segment, physical base 0x0EFC0)
- CGA VRAM at 0xB8000

Physical address of DS variable `[0xXXXX]` = 0x0EFC0 + 0xXXXX.

---

## Key Data Variables (DS-relative)

| Address  | Purpose                                      |
|----------|----------------------------------------------|
| [0x0004] | Game state / scene index (always 0x0000)     |
| [0x06A1] | Scan code lookup table (22 entries)          |
| [0x06B7] | Key state table (0x00=pressed, 0x80=released)|
| [0x06B9] | Right arrow key state (index 2 in table)     |
| [0x0698] | Horizontal movement direction (0=none, 1=right, 2=left) |
| [0x0699] | Vertical movement direction                  |
| [0x069F] | Last INT 1A tick for keyboard rate gate      |
| [0x1673] | Player-active flag (0=attract mode, non-0=player control) |
| [0x5920] | Scroll pause flag                            |
| [0x5925] | Last INT 1A tick for movement timer          |
| [0x592A] | Scroll distance counter (starts at 0x0500)  |
| [0x592C] | Scroll direction                             |

---

## INT 09h Keyboard Handler - CS:0x14B3

The game installs a custom IRQ1 handler. Verified it works correctly:

1. Reads scan code from port 0x60
2. Looks up scan code in 22-entry table at DS:0x06A1
3. Stores 0x00 (pressed) or 0x80 (released) in parallel state table at DS:0x06B7

Right arrow scan code = 0x4D, stored at [0x06B9] (table index 2).

**Log evidence (line 682724):** `pushing key 0x4D` → handler executes → `[0x06B9]=0x00` (pressed). Working correctly.

---

## Keyboard Aggregator - CS:0x12C1

Called from CS:0x1200 (timer-gated, requires 2 INT 1A ticks between calls).

Reads raw key states, applies joystick masking, XORs with 0x80, and writes:
- [0x0698] = 0x01 when right arrow pressed
- [0x0698] = 0x02 when left arrow pressed
- [0x0699] = direction for up/down

**Log evidence (line 697211):** `mov [0x0698], al` with `AX=0001` — correctly sets move-right flag.

---

## Main Loop - CS:0x0155-0x0191

```
0x0155: check [0x1F80]
0x015F: call 0x1338  ; scroll sync check
0x0162: check [0x041C]
0x016C: check [0x041B]
0x0176: call 0x1200  ; keyboard input (timer-gated)
0x0179: call 0x08E5  ; delay / frame timer
0x017F: check [0x1CB8]
0x0184: jnz 0x0191
0x0186: inc [0x040F]
0x018A: test [0x040F], 0x03  ; every 4th iteration...
0x018F: jnz 0x0155
0x0191: call 0x546D  ; MAIN MOVEMENT FUNCTION
```

The movement function CS:0x546D is called every 4th main loop iteration when
[0x1CB8]=0.

---

## Main Movement Function - CS:0x546D

This is the central function. It runs the movement/scroll logic for the current scene.

### Critical branch at CS:0x5534-0x5549 (speed/mode selection):

```asm
5534: mov si, 0x0001       ; default si=1 (player speed)
5537: cmp [0x0004], 0x0000
553C: jnz 0x5549           ; if state != 0, keep si=1
553E: dec si               ; si = 0
553F: cmp [0x1673], 0x00
5544: jz 0x5549            ; if [0x1673]==0, keep si=0 (attract mode)
      ; if [0x1673]!=0, si stays 1 (player mode)
5549: mov di, si
554B: shl di, 1            ; di = si * 2  (table index)
```

**Outcome with [0x0004]=0 and [0x1673]=0:**
- si = 0 (attract/demo mode scroll speed)
- di = 0 (uses slow auto-scroll timing table)

**Outcome with [0x0004]=0 and [0x1673]!=0:**
- si = 1 (player-controlled speed)
- di = 2 (uses player timing table)

### Timer threshold check (CS:0x555C):

```asm
5556: mov ax, dx           ; current INT 1A tick
5558: sub ax, [0x5925]     ; elapsed since last scroll
555C: cmp ax, [di+0x59F2]  ; compare to threshold
5560: jnb 0x5563           ; if elapsed >= threshold, do scroll
5562: ret                  ; else return (no movement this frame)
```

- When di=0: threshold = [0x59F2] = 3 ticks
- When di=2: threshold = [0x59F4] = ? ticks (player mode)

---

## Root Cause: Game is in Attract Mode

**[0x1673] is ALWAYS 0x00** throughout the entire 1M-line execution log.

This flag is the "player-active" flag. When it is 0, CS:0x546D selects attract-mode
scroll (si=0, di=0), which auto-scrolls the alley independent of player input. The
player's [0x0698] movement direction byte is computed correctly but is **never read** by
the scroll function when in attract mode.

The function at CS:0x1B7A (called from main loop at 0x0BAC when [0x0004]=0) checks
[0x1673] and immediately returns if 0:

```asm
1B7A: cmp [0x0004], 0x0000
1B7F: jnz 0x1BE1
1B81: mov dl, [0x1673]
1B85: cmp dl, 0x00
1B88: jz 0x1BE1            ; returns immediately if [0x1673]==0
```

### What Should Set [0x1673]?

[0x1673] is initialized to 0x00 at CS:0x1835 (level setup). It is never written to a
non-zero value anywhere in the log. In Alley Cat, pressing SPACE/FIRE is supposed to
transition from attract/demo mode to player-controlled mode, which likely sets [0x1673].
That transition is not happening.

---

## Scroll Counter Status

The horizontal scroll counter [0x592A] starts at 0x0500 (1280) and decrements by
0x19 (25) each time the timer fires. Observed values in the log:

| Log line | Value  | Delta |
|----------|--------|-------|
| 596578   | 0x0500 | init  |
| 596938   | 0x04E7 | -25   |
| 667534   | 0x04CE | -25   |
| 759217   | 0x04B5 | -25   |
| 851234   | 0x049C | -25   |
| 947463   | 0x0483 | -25   |

Only 5 decrements in 1M lines. 1280/25 = ~51 decrements needed to complete the alley
scroll. The log would need ~10M lines to see the scroll complete. The alley auto-scroll
IS happening, just slowly.

---

## Secondary Issues Found

### 1. CRT Register 0x02 Never Written

Function CS:0x1580 writes to CGA CRT register 0x02 (horizontal scroll). It is **never
executed** in the 1M-line log (`grep -n "OP 160F:1580"` returns nothing).

Additionally, `video_card.rs` `io_write_u8` for `VIDEO_CARD_DATA_ADDR` only handles
registers 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F. Register 0x02 falls to `_ => log::warn!`
and is silently discarded. If CS:0x1580 ever executes, the scroll write will be lost.

### 2. Decoder Bug for Opcode 0xA0

`InstructionDecoder` for opcode 0xA0 (`MOV AL, [offset16]`) always sets
`segment_override: None` in the MemoryRef, regardless of any active segment override
prefix (e.g., ES prefix 0x26). This means the exec log `@SEG:OFF` annotation shows
the wrong segment (uses DS instead of ES) for these instructions. This is a display-only
bug — the actual execution uses the correct segment via `self.segment_override`.

Affected file: [core/src/cpu/instructions/decoder.rs](../core/src/cpu/instructions/decoder.rs)

---

## Summary of Code Path for Right Arrow Press

```
1. Port 0x60 read → scan code 0x4D                          [WORKS]
2. INT 09h handler at CS:0x14B3                              [WORKS]
3. [0x06B9] = 0x00 (right arrow pressed)                    [WORKS]
4. CS:0x1200 (timer gate) → calls CS:0x12C1                 [WORKS]
5. CS:0x12C1 reads [0x06B9]=0, sets [0x0698]=0x01           [WORKS]
6. CS:0x546D called from main loop                           [WORKS]
7. CS:0x5537: [0x0004]=0 → dec si                           [WORKS]
8. CS:0x553F: [0x1673]=0 → jz (si stays 0, attract mode)   [PROBLEM]
9. Player input in [0x0698] is NEVER consulted in state 0   [ROOT CAUSE]
```

---

## Hypotheses for Fix

1. **[0x1673] not being set**: Find what should write [0x1673] to non-zero when the
   player presses a start key. Likely triggered by SPACE or FIRE key press. The INT 09h
   handler or a separate "start game" key check may be broken.

2. **Attract mode is intentional but start key doesn't work**: The game may require
   pressing SPACE/ENTER to "insert coin" / start. If that scan code isn't in the 22-entry
   lookup table, the key state would never be detected and [0x1673] never set.

3. **Timer too slow**: The PIT is reprogrammed to ~140Hz. If the emulator's INT 1A
   time counter doesn't tick fast enough relative to the reprogrammed PIT rate, the
   game's timer-gated code runs far less often than it should.
