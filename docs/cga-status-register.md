# CGA Status Register (Port 0x3DA)

## Hardware Behavior

The real IBM CGA card exposes a status register at I/O port 0x3DA with two key bits:

| Bit | Meaning |
|-----|---------|
| 0   | Horizontal retrace active (1 = CPU may write to video RAM without snow) |
| 3   | Vertical retrace active (1 = frame boundary, safe for full-screen updates) |

On real CGA hardware the timing is driven by the video scan:
- **Horizontal retrace** (bit 0): ~15% of each scan line, ~15.7 kHz
- **Vertical retrace** (bit 3): ~8% of each 60 Hz frame (~1.4 ms per 16.7 ms frame)

Programs use these bits to synchronise video memory writes and avoid visual artefacts:

```asm
; Common pattern: wait for vsync start (to sync on frame boundary)
wait_active:
    in  al, 03DAh
    test al, 08h
    jnz wait_active     ; spin while still in vsync

wait_retrace:
    in  al, 03DAh
    test al, 08h
    jz  wait_retrace    ; spin until vsync starts

; Now write to video memory before vsync ends
```

```asm
; Hsync-safe write pattern (avoids CGA "snow")
wait_hsync_end:
    in  al, 03DAh
    test al, 01h
    jnz wait_hsync_end  ; spin while in hsync

; Safe window to write one byte/word
```

## Current Emulator Implementation

Port 0x3DA is **not accurately emulated** — real timing is not simulated.

Instead, a simple counter (`cga_status_counter: u32` in `IoDevice`) increments on every read of the port. The returned value alternates on each read:

- **Even reads** → `0x00` (neither retrace active)
- **Odd reads**  → `0x09` (bits 0 and 3 set: both hsync and vsync active)

```rust
// core/src/io/mod.rs
0x3DA => {
    self.cga_status_counter = self.cga_status_counter.wrapping_add(1);
    if self.cga_status_counter & 1 == 0 { 0x00 } else { 0x09 }
}
```

This ensures both "in retrace" and "not in retrace" states are seen within two consecutive reads, so all standard wait patterns exit promptly without getting stuck.

## Why the Naive Return Value of 0xFF Breaks Programs

Before this fix, port 0x3DA was unhandled and fell through to the default `0xFF`. With bits 0 and 3 permanently set, any program doing "wait until NOT in retrace" (`jnz` loop) would spin forever, causing a hang. MS Flight Simulator 1.05 exhibited exactly this: it drew approximately 90 scan lines of graphics and then stalled waiting for the retrace to end before writing the next batch.

## Known Limitations

- **No cycle-accurate timing**: a real CGA card has strict timing windows. Programs that depend on precise retrace duration (e.g. those that measure frame rate by counting retraces, or those that stream data into video RAM during the exact retrace window) may behave differently from real hardware.
- **Both bits toggle together**: on real hardware hsync and vsync are independent. Programs that test only one bit are unaffected, but programs that rely on the two bits being independent could see incorrect behaviour.

## Potential Future Improvement

Track emulated CPU cycles and derive the retrace bits from elapsed time relative to the 18.2 Hz tick counter, matching the real 60 Hz vsync and ~15.7 kHz hsync periods.
