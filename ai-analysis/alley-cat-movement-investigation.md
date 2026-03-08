# Alley Cat (CAT.EXE) - Cat Movement Investigation

## Problem Statement

Pressing the right arrow key does not cause the cat to move visibly. Investigation traced
the full code path from keyboard input through to the movement/scroll mechanism.

---

## Disassembler

The disassembly of `cat.exe` is checked in at [exe-analysis/cat.asm](../exe-analysis/cat.asm).

To regenerate it after decoder changes or to add new labels/comments:

```bash
cargo run -p oxide86-disasm -- --config exe-analysis/cat.json exe-analysis/cat.exe > exe-analysis/cat.asm
```

The config file [exe-analysis/cat.json](../exe-analysis/cat.json) controls the disassembly:

```json
{
  "loadSegment": "0EEC",
  "dataSegment": "0EFC",
  "entryPoints": {
    "160F:XXXX": "label_name"
  },
  "comments": {
    "160F:XXXX": "what this instruction does"
  },
  "data": {
    "0EFC:6AA0": { "type": "string", "label": "str_joystick_prompt" }
  }
}
```

**`loadSegment`** — set to `0EEC` so all CS:IP addresses in the output match the emulator's execution logs (CS = `160F`, not `0723`).

**`dataSegment`** — set to `0EFC` (the DS value at runtime). Required for the disassembler to resolve `data` entries to physical addresses.

**`data`** — map of `"SEG:OFF"` → `{ "type", "label" }` for annotating known data regions (strings, byte tables, etc.) with labels in the disassembly output.

To investigate a specific function, search the `.asm` for the CS:IP from the execution log directly — addresses will match exactly.

---

## Runtime Memory Layout

- CS = 0x160F (code segment)
- DS = 0x0EFC (data segment, physical base 0x0EFC0)
- CGA VRAM at 0xB8000
- Relocation confirmed: `B8 10 00` in binary → `B8 FC 0E` at runtime (`mov ax, 0x0EFC`, not `0x0010`)

Physical address of DS variable `[0xXXXX]` = 0x0EFC0 + 0xXXXX.

---

## Key Data Variables (DS-relative)

| Address  | Purpose                                                                    |
|----------|----------------------------------------------------------------------------|
| [0x0004] | Game state / scene index (always 0x0000)                                  |
| [0x057b] | Cat screen Y position (0=top, 0x60=window level, 0xB4=ground, max 0xB3)  |
| [0x0571] | Previous vertical direction (1=DOWN, 0xFF=UP)                             |
| [0x0572] | Current horizontal cat position/speed (updated by 0x0FC9)                 |
| [0x0574] | Horizontal position accumulator                                            |
| [0x0579] | Horizontal movement direction (1=right, 0xFF=left)                        |
| [0x0593] | Horizontal position accumulator (alley cat X in alley)                    |
| [0x0693] | Keypress counter (incremented on any key down in INT 09h handler)         |
| [0x06A1] | Scan code lookup table (22 entries, see below)                            |
| [0x06B7] | Key state table (0x00=pressed, 0x80=released, parallel to scan_code_table)|
| [0x06B9] | Right arrow key state (table index 2, scan code 0x4D)                    |
| [0x0698] | Horizontal movement direction from aggregator (1=right, 0xFF=left)        |
| [0x0699] | Vertical movement direction from aggregator (1=down, 0xFF=up)             |
| [0x069F] | Last INT 1A tick for keyboard rate gate                                   |
| [0x1665] | Animation frame counter for window-entry sequence                         |
| [0x1666] | Window entry sub-counter                                                  |
| [0x1668] | Target building floor number                                              |
| [0x1670] | Building-entry-triggered flag (set to 1 when cat reaches window)          |
| [0x1673] | Cat building floor number (0=in alley/ground, non-zero=floor in building) |
| [0x1CBF] | Mode flag (when non-zero: overrides si to 3 in movement function)         |
| [0x5920] | Scroll pause flag                                                          |
| [0x5925] | Last INT 1A tick for movement timer                                       |
| [0x592A] | Scroll distance counter / sound frequency control                         |
| [0x592C] | Scroll direction                                                          |
| [0x5B07] | Mode flag (when non-zero: overrides si to 3 in movement function)         |

---

## Scan Code Table (DS:0x06A1, 22 entries)

The INT 09h handler searches this table using REPNE SCASB. Only these keys are recognized:

| Index | Scan Code | Key     |
|-------|-----------|---------|
| 0     | 0x38      | Alt     |
| 1     | 0x48      | Up      |
| 2     | 0x4D      | Right   |
| 3     | 0x50      | Down    |
| 4     | 0x4B      | Left    |
| 5     | 0x49      | PgUp    |
| 6     | 0x51      | PgDn    |
| 7     | 0x4F      | End     |
| 8     | 0x47      | Home    |
| 9     | 0x01      | Esc     |
| 10    | 0x15      | Y       |
| 11    | 0x31      | N       |
| 12    | 0x25      | K       |
| 13    | 0x23      | H       |
| 14    | 0x14      | T       |
| 15    | 0x1E      | A       |
| 16    | 0x1F      | S       |
| 17    | 0x13      | R       |
| 18    | 0x1D      | Ctrl    |
| 19    | 0x53      | Del     |
| 20    | 0x32      | M       |
| 21    | 0x0A      | 9       |

**SPACE (0x39) and ENTER (0x1C) are NOT in the scan code table.** The game exits attract
mode by detecting any key via [0x0693] (incremented on any keydown in INT 09h), not by a
specific key. Pressing any recognized key increments this counter.

---

## INT 09h Keyboard Handler - CS:0x14B3

The game installs a custom IRQ1 handler. Verified it works correctly:

1. Sets ES = 0x0EFC (data segment, using relocated selector)
2. Reads scan code from port 0x60
3. Searches scan_code_table with REPNE SCASB
4. Stores 0x00 (pressed) or 0x80 (released) in parallel state table at ES:0x06B7
5. **Always increments [0x0693] on any key press** (regardless of whether key is in table)

Right arrow scan code = 0x4D, stored at [0x06B9] (table index 2).

**Log evidence (line 682724):** `pushing key 0x4D` → handler executes → `[0x06B9]=0x00` (pressed). Working correctly.

---

## Keyboard Aggregator - CS:0x12C1

Called from CS:0x1200 (timer-gated, requires 2 INT 1A ticks between calls).

Reads raw key states, applies joystick masking, XORs with 0x80, and writes:
- [0x06BA] (DOWN index 3) → [0x0699] = 1 (down)
- [0x06B8] (UP index 1) → [0x0699] = 0xFF (up)
- [0x06B9] (RIGHT index 2) → [0x0698] = 1 (right)
- [0x06BB] (LEFT index 4) → [0x0698] = 0xFF (left)

**Log evidence (line 697211):** `mov [0x0698], al` with `AX=0001` — correctly sets move-right flag.

---

## Cat Screen Position - [0x057b]

[0x057b] is the cat's Y position on screen:
- `0x00` = top of screen
- `0x60` = window-level threshold (96 pixels from top)
- `0xB4` = ground level (180 pixels from top) — cat starts here in main game
- Clamped to range `0x04..=0xB3`

UP arrow (0x0699=0xFF) **decreases** [0x057b] (cat jumps upward).
DOWN arrow (0x0699=1) **increases** [0x057b] (cat falls).

The vertical update logic is at CS:0x0A86–0x0ACE:
```
0A86: load [0x0571] (prev vertical dir)
      if 1 (DOWN): [0x057b] += speed
      if 0xFF (UP): [0x057b] -= speed
      clamp to 0x04..0xB3
```

Level init at CS:0x0733: `mov [0x057b], 0xB4` — cat starts at ground.
Attract-mode setup at CS:0x5D1F: `mov [0x057b], 0x60` — places cat at window height.

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
0x0197: call 0x1936  ; window-entry check (every 13 main-movement calls)
```

The movement function CS:0x546D is called every 4th main loop iteration when [0x1CB8]=0.
The window-entry check (CS:0x1936) is called on a separate counter.

---

## Main Movement Function - CS:0x546D

This is the central function. It runs the movement/scroll logic for the current scene.

### Speed/mode selection (CS:0x5522–0x5549):

```asm
5522: mov si, 0x0003       ; default si=3 (fast mode)
5527: cmp [0x1CBF], 0x00
      jnz 0x5549           ; if [0x1CBF]!=0 keep si=3
      cmp [0x5B07], 0x00
      jnz 0x5549           ; if [0x5B07]!=0 keep si=3
5534: mov si, 0x0001       ; si=1 (player-speed candidate)
5537: cmp [0x0004], 0x0000
553C: jnz 0x5549           ; if state != 0, keep si=1
553E: dec si               ; si = 0
553F: cmp [0x1673], 0x00
5544: jz 0x5549            ; if [0x1673]==0, keep si=0 (attract/ground mode)
      inc si               ; si = 1 → shl → si=2 (player in-building mode)
5549: mov di, si
554B: shl di, 1            ; di = si * 2  (table index)
```

**Outcome with [0x0004]=0, [0x1673]=0 (cat on ground, attract/main alley):**
- si = 0 → di = 0 (slow auto-scroll timing, player input in [0x0698] ignored by scroll)

**Outcome with [0x0004]=0, [0x1673]!=0 (cat inside a building):**
- si = 2 → di = 4 (player-controlled in-building timing)

### Timer threshold check (CS:0x555C):

```asm
5556: mov ax, dx           ; current INT 1A tick
5558: sub ax, [0x5925]     ; elapsed since last scroll
555C: cmp ax, [di+0x59F2]  ; compare to threshold
5560: jnb 0x5563           ; if elapsed >= threshold, do scroll
5562: ret                  ; else return (no movement this frame)
```

- When di=0: threshold = [0x59F2] = 3 ticks (attract-mode auto-scroll)
- When di=4: threshold = [0x59F6] = ? ticks (player in-building mode)

---

## Cat Building Floor Variable - [0x1673]

**[0x1673] is the cat's building floor number, NOT a generic "player-active flag".**

- `0` = cat is in the alley (ground level, main scrolling scene)
- Non-zero = cat is inside a building on that floor number

When [0x1673]=0, CS:0x546D selects si=0 (attract/auto-scroll). The player's [0x0698]
movement direction byte is computed correctly but the **scroll function ignores it** when
[0x1673]=0 (cat still on the ground).

[0x1673] is initialized to 0x00 at CS:0x1835 (level_setup). It is set to a non-zero value
at CS:0x1A26 only after the cat successfully enters a building window.

---

## Window Entry Mechanism - CS:0x1936

This is how [0x1673] gets set (the transition from alley to building):

```
1. sub_17A26 at 0x1936: called from main loop every ~13 movement calls
2. 0x195F: cmp [0x057b], 0x60; ja 0x193c  → cat must be at/above window (Y <= 0x60)
3. 0x19C8: mov [0x1670], 0x01             → set building-entry-triggered flag
4. sub_17BF5 at 0x1B05: validates window alignment
   0x1B30: cmp [0x057b], 0x60; jnb 0x1B4A → requires Y STRICTLY < 0x60 (not just <=)
5. Countdown [0x1665] → timing check at 0x1A03
6. 0x1A26: mov [0x1673], al              → al loaded from [0x1668] (target floor)
```

**For [0x1673] to become non-zero, the cat must jump UP (UP arrow) until [0x057b] < 0x60.**
The cat starts at [0x057b]=0xB4 (ground). Only sustained UP arrow presses will raise the
cat to window level and trigger the building-entry sequence.

RIGHT arrow alone cannot set [0x1673]. It moves the cat horizontally but doesn't affect Y.

---

## player_active_check - CS:0x1B7A

```asm
1B7A: cmp [0x0004], 0x0000
1B7F: jnz 0x1BE1
1B81: mov dl, [0x1673]
1B85: cmp dl, 0x00
1B88: jz 0x1BE1            ; returns immediately if [0x1673]==0 (cat on ground)
```

This function gates additional cat-position logic behind [0x1673]!=0 (cat in building).
It returns early when the cat is still in the alley.

---

## Attract Mode Sub-Loop - CS:0x5D54

Separate from the main loop. Runs when the game first starts (or between levels).

- CS:0x5D1F: `mov [0x057b], 0x60` — places demo cat at window height
- CS:0x5D54–0x5DD3: loop body checks [0x0693] for any key press to exit
- CS:0x5DA7–0x5DAA: calls `0x53B0` (scroll) then `0x5DD4` (AI movement)
- Exit condition: `cmp [0x0693], 0x00; jnz exit_attract` — any key press exits

---

## Attract-Mode Cat AI - CS:0x5DD4

Sets [0x0698] based on [0x0579] (left/right movement), does NOT set [0x0699].
Ends with a vsync check:

```asm
5E1A: call 0x13D8          ; check vsync (port 0x3DA bit 3)
5E1D: jz 0x5E2A            ; if NOT in vsync, skip to ret
      ; (vsync-gated: sets [0x0572], calls frame timer)
5E2A: ret
```

The sub-routine `0x13D8` reads port 0x3DA and checks bit 3 (vsync flag). Since the
emulator always returns 0x00 from port 0x3DA (see CGA Vsync Bug below), ZF is always 1,
so **the code always jumps to ret**. The vsync-synchronized movement update that would
set [0x0572] and drive the frame timer is never executed in attract mode.

---

## CGA Vsync Bug (Port 0x3DA)

**Root cause of attract-mode animation not executing.**

Port 0x3DA (Input Status Register 1) must toggle between 0x00 (not in vsync/retrace) and
0x08 (in vsync) at ~60Hz. The emulator's `video_card.rs` always returns `Some(0x00)`:

```rust
// Input Status Register 1: resets AC flip-flop to address mode
0x3DA => {
    self.ac_flip_flop.set(false);
    Some(0x00)  // BUG: always returns 0, never simulates vsync
}
```

Consequence: any code that polls 0x3DA waiting for vsync (bit 3 = 1) will never see it.
Any code that branches on vsync state always takes the "not in vsync" branch.

For attract-mode cat AI at 0x5DD4: `call 0x13D8; jz 0x5E2A` → always jumps to ret,
skipping the movement application block. The cat never moves in attract mode.

Additionally, CRT register 0x02 (horizontal fine scroll, written by `crt_horiz_scroll`
at CS:0x1580) falls to `_ => log::warn!` in `video_card.rs` and is silently discarded.

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
8. CS:0x553F: [0x1673]=0 → si stays 0 (cat is on ground)   [ROOT CAUSE]
9. Player input in [0x0698] is not consulted by scroll       [CONSEQUENCE]
10. Cat moves horizontally ([0x0579]) but scroll ignores it  [CONSEQUENCE]
```

RIGHT arrow DOES move the cat horizontally in the alley (updates [0x0579], [0x0574],
[0x0572]). However the scroll rate stays at attract-mode speed (3 ticks) regardless of
player input. The cat's position relative to the scrolling background may not visibly
change.

---

## Hypotheses for Fix

1. **[0x1673] not being set**: The game requires pressing UP to jump the cat toward a
   window. [0x0057b] must drop below 0x60. UP arrow is the correct key (0x48, index 1
   in scan_code_table). This part is working — the issue is the vsync bug prevents
   the cat from moving in attract mode, so the cat may not appear to respond to UP either.

2. **CGA vsync (port 0x3DA) not simulated**: ~~CONFIRMED BUG.~~ Port 0x3DA always
   returns 0x00. The attract-mode cat AI (0x5DD4) polls vsync via 0x13D8 and always
   skips its movement block. **Fix**: Toggle bit 3 of port 0x3DA at ~60Hz in
   `video_card.rs` to simulate vsync. This should fix attract-mode cat movement.
   The main game loop movement (0x546D) does not poll vsync directly.

3. **~~Timer too slow (PIT at ~140Hz)~~**: INCORRECT. The game does NOT reprogram PIT
   channel 0 (port 0x40). Only PIT channel 2 (port 0x42, PC speaker sound) is
   reprogrammed by the game. INT 1Ah timer ticks at the correct 18.2Hz.

---

## PIT / Timer Timing

- PIT channel 0 divisor: 65536 (default, NOT changed by game) → 18.2Hz IRQ rate
- INT 08h: increments BDA timer counter at 18.2Hz (correct)
- INT 1Ah (AH=00h): game reads DX (low word of tick count) for timing gates
- PIT channel 2 (port 0x42): reprogrammed by game for PC speaker sound effects
- `counter_value_ch0` (pit.rs): used by CS:0x13B7 to read current channel 0 counter
  value for timing delays (not for IRQ rate)
- LFSR seed at CS:0x2E10: reads PIT ch0 counter at startup, uses fallback 0xFA59 if zero
