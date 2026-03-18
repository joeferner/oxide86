# Commander Keen 1 – Laggy Scrolling Investigation

**Date:** 2026-03-17

## Symptom

Scrolling appears visually laggy even though the performance overlay confirms the
emulator is running at the target speed (16 MHz).

## How the Game Scrolls

Commander Keen 1 uses **hardware page-flipping via the CRTC start address** rather
than copying pixel data. The mechanism is:

1. The game renders the next frame into an off-screen region of video RAM.
2. It then writes the new display start address to the CRTC by sending two
   register/value pairs through the standard 6845 port pair:

   | Port | Value | Meaning |
   |------|-------|---------|
   | 0x3D4 | 0x0C | Select Start Address High register |
   | 0x3D5 | high byte | Upper 8 bits of new start address |
   | 0x3D4 | 0x0D | Select Start Address Low register |
   | 0x3D5 | low byte | Lower 8 bits of new start address |

3. The entire write sequence is performed **twice in a row** (routine at
   `102A:B833`–`B844`, repeated at `102A:B851`–`B862`). This double-write is a
   known CGA-era reliability practice to guard against hardware timing glitches.

Observed start address values across two separate sessions confirm real page flips
are occurring (e.g. `0x3784` → `0x0785`).

## Root Cause Hypothesis

The scroll update is **gated by a software flag** stored at DS:`[0x5652]`. In the
captured log this flag reads `0x0001`, which causes the game to enter its scroll
update path (`jne 0xB7B1` → `call far 1E96:004D`).

The flag is almost certainly set by an **interrupt handler**, not by polling the
VBlank status port. Evidence:

- No `in al, 0x3DA` (VGA Input Status 1 – vertical retrace bit) reads appear in
  the execution log anywhere near the CRTC writes. Games that sync to VBlank
  universally poll this port in a tight spin loop before flipping the start
  address.
- The flag pattern (`[0x5652] = 1`, tested each game loop iteration, cleared after
  processing) is the classic "interrupt set / main loop consume" handshake used
  with PIT/IRQ0.

**If the flag is set by IRQ0 (PIT channel 0 at ~18.2 Hz)**, the scroll update runs
at approximately 18 frames per second regardless of CPU speed. This is inherently
visible as stutter even when the CPU is running at full speed.

## Emulator Behaviour

The emulator handles the CRTC start address correctly:

- `set_start_address()` in `video_buffer.rs` stores the new address and sets
  `dirty = true` immediately.
- The renderer reads `start_address` when `render_and_clear_dirty()` is called at
  the end of each display frame.
- VSync is simulated at 60 Hz via CPU cycle count (port 0x3DA bit 3).

The emulator itself does not introduce additional scroll lag; the bottleneck is
entirely in the game's own update cadence.

## Next Steps

1. **Confirm the interrupt source.** Set a breakpoint or log writes to the IRQ0
   vector (IVT entry at `0x0020`–`0x0023`) and the PIT counter value. If IRQ0 is
   firing at 18.2 Hz and its handler sets `[0x5652]`, that confirms the 18 fps
   scroll rate.

2. **Check whether Keen reprograms the PIT.** Some id Software DOS games reprogram
   PIT channel 0 to a higher rate (e.g. 70 Hz) for smoother animation. If Keen
   does this but the emulator's PIT is not generating IRQs at the programmed rate,
   the scroll would be slower than intended.

3. **Verify PIT IRQ delivery.** Confirm that PIC IRQ0 interrupts are actually being
   delivered to the CPU at the PIT-programmed rate. A mis-timed or missed IRQ0
   would slow the flag-set cadence and make scrolling appear sluggish.

## Summary

| Aspect | Detail |
|--------|--------|
| Scroll method | CRTC Start Address page-flip (regs 0x0C/0x0D) |
| Update trigger | Software flag at DS:`[0x5652]`, likely set by IRQ0 |
| Likely update rate | ~18.2 Hz (default PIT) or reprogrammed rate |
| Emulator fault | None identified – lag is in game update cadence |
| Most likely fix | Ensure PIT reprogramming and IRQ0 delivery are correct |
