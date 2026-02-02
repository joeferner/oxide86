# Fix QBASIC PLAY Sound Issues

## Problem 1: Notes Too Long

The user reports notes play for too long. Looking at the timing:
- The emulator runs at ~4.77 MHz emulated CPU speed
- Each instruction is estimated at 10 cycles
- Timer ticks fire every 262088 cycles (18.2 Hz)

QBASIC PLAY likely uses the BIOS timer (INT 1C hooks or polling BDA timer counter) for note duration.
The issue is that the emulator runs instructions faster than a real 8086, but the *emulated*
time (cycle count) progresses correctly. This means notes play for the correct number of
emulated cycles but in real wall-clock time they're too fast (not too long).

Wait - user says "too long" which means the opposite. Let me reconsider:
- If INT 15h AH=86h returns immediately, timing would be too FAST (notes too short)
- "Too long" suggests QBASIC is using a different timing mechanism

Actually "too long" likely means the notes sustain longer than expected - QBASIC PLAY has tempo
settings (T180 = 180 beats per minute) and note length (L8 = eighth notes). If these play slower
than expected, it could be:
1. The emulator's cycle timing is too slow in real-time
2. QBASIC is busy-waiting in some way that runs slower

Need to investigate QBASIC's timing mechanism more.

## Problem 2: High-Pitched Sound Between Notes

In `update_speaker()`:
```rust
let count = self.io_device.pit().get_channel_count(2);
if count > 0 {
    let frequency = 1193182.0 / (count as f32);
    self.speaker.set_frequency(true, frequency);
}
```

When QBASIC programs a new note:
1. Write command byte (0xB6) to port 0x43 → PIT sets `null_count = true`
2. Write LSB to port 0x42
3. Write MSB to port 0x42 → `count_register` updated, `null_count = false`

The issue: `get_channel_count()` returns `count_register` which might be:
- Still the old value during transition
- Or the wrong value if only LSB was written (in access_mode 3)

Additionally, when the PIT is first initialized or between notes, `count_register` might be 0
which is treated as disabled, but that should give silence, not high pitch.

More likely: When QBASIC disables the speaker between notes (clears port 0x61 bits),
and then re-enables it for the next note, there's a brief moment where:
1. Speaker enabled (port 0x61 bits set)
2. But PIT command byte just written (old count still used)
3. High frequency played briefly before new count loaded

## Solution

### Fix 1: Check PIT null_count before enabling speaker

Add a method to PIT to check if channel 2 has a valid count:
```rust
pub fn is_channel_ready(&self, channel: u8) -> bool {
    !self.channels[channel as usize].null_count
}
```

Then in `update_speaker()`:
```rust
if enabled && self.io_device.pit().is_channel_ready(2) {
    ...
}
```

### Fix 2: Performance optimizations (for timing)

The "notes too long" issue is likely caused by the emulator running slower than real-time due to overhead.

Implemented optimizations:
1. Reduced speaker update frequency from every instruction to every ~100 cycles
2. Added frequency caching in RodioSpeaker to avoid unnecessary mutex locks
3. Fixed count=0 case to properly treat it as 65536 (lowest frequency ~18.2 Hz)

## Implementation Summary

### Changes Made:

1. **core/src/io/pit.rs**:
   - Added `is_channel_ready()` method to check if channel has a valid count loaded

2. **core/src/computer.rs**:
   - Added `speaker_update_cycles` counter field
   - Modified `update_speaker()` to check `pit_ready` before enabling speaker
   - Modified `increment_cycles()` to only update speaker every ~100 cycles
   - Fixed count=0 handling to output 18.2 Hz instead of disabling

3. **core/src/rodio_speaker.rs**:
   - Added `last_frequency` caching field
   - Modified `set_frequency()` to skip mutex lock when frequency unchanged
