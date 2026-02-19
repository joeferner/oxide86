# OPL2 FM Synthesis Fixes

**Session date**: 2026-02-19
**File**: `core/src/audio/opl2.rs`
**Goal**: Make AdLib jukebox instruments sound correct — accurate timbre, volume balance, and drums.

---

## Summary of Changes Made

### 1. Logarithmic (dB-based) attenuation in `sample_at_phase` ✅

**Problem**: TL register was applied linearly (`volume = 1 - tl/1023`), making quiet instruments
too loud and loud instruments too quiet.

**Fix**: Convert TL and envelope to dB then use `10^(-dB/20)`:
```rust
let tl_db  = self.total_level as f32 * 0.75;      // 6-bit, 0.75 dB/step → 0–47.25 dB
let env_db = self.env_level   as f32 * (47.25 / 1023.0);
let total_db = tl_db + env_db;
let volume = if total_db >= 96.0 { 0.0 } else { 10.0f32.powf(-total_db / 20.0) };
```

**Tremolo**: Changed from `±6 % linear` to `±4.8 dB` per hardware spec:
```rust
volume * 10.0f32.powf(-tremolo_amp * 4.8 / 20.0)
```

---

### 2. FM phase modulation depth: `<< 4` → `<< 8` ✅

**Problem**: `mod_out << 4` gave only ~0.5 cycles of phase shift — instruments sounded like
pure sine tones instead of their intended FM timbre.

**Fix**: Changed to `mod_out << 8` in both the melodic channel loop and the bass drum rhythm path.

**Why `<< 8`**: In Nuked-OPL3, a full-volume modulator output (~±8192) is added directly to the
10-bit phase index (1024 entries), giving ±8 full cycles of phase shift. Our full-scale output is
±32767. `32767 >> 10 ≈ 31`; `31 << 8 ≈ 8192` — matching hardware depth.

Affected lines:
- Melodic FM path: `let phase_mod = mod_out << 8;`
- Bass drum (rhythm mode): `op.calc_output(mod_out << 8, waveform_enable, tremolo)`

---

### 3. Mixing normalization ✅

**Problem**: `mix.clamp(-32767, 32767)` clipped with two or more simultaneous channels.

**Fix**: Allow ~3 channels before clipping:
```rust
mix.clamp(-32767 * 3, 32767 * 3) as f32 / (32767.0 * 3.0)
```

---

### 4. Rhythm mode: phase-based drum synthesis (Nuked-OPL3 style) ✅

**Problem**: Hi-hat, snare, cymbal were mixing noise amplitude with operator output
(hardcoded `noise * 12000.0`). Real OPL2 uses phase manipulation, not amplitude noise.

**Fix**: Rewrote rhythm synthesis using Nuked-OPL3 phase formulas:
```
rm_xor = (hh_bit2 ^ hh_bit7) | (hh_bit3 ^ tc_bit5) | (tc_bit3 ^ tc_bit5)
hh_idx = rm_xor<<9 | (0xd0 if rm_xor≠noise_bit else 0x34)
sd_idx = hh_bit8<<9 | ((hh_bit8^noise_bit)<<8)
tc_idx = rm_xor<<9 | 0x80
```

Phase bits are extracted from hi-hat operator (slot 13) and cymbal operator (slot 17).
10-bit indices are converted to 20-bit phase space via `<< 10`.

**LFSR fix**: Changed from shift-left with wrong taps to Nuked-OPL3 style (shift-right, taps at bits 14 and 0):
```rust
let n_bit = ((self.noise_lfsr >> 14) ^ self.noise_lfsr) & 1;
self.noise_lfsr = (self.noise_lfsr >> 1) | (n_bit << 22);
```

---

### 5. Exponential (curved) attack envelope ✅

**Problem**: Attack was linear (`env_level -= inc`). Real OPL2 uses `~eg_rout >> shift`
which creates a concave curve — large steps from silence, slowing near full volume.

**Fix**: `inc = (env_level >> shift).max(1)` where shift is derived from attack rate:

| Rate | Shift | Character |
|------|-------|-----------|
| 14   | 0     | Very fast (halves each sample) |
| 12   | 2     | Fast |
| 8    | 6     | Medium |
| 4    | 10    | Slow |
| 1    | 14    | Very slow |
| 15   | —     | Instant |
| 0    | —     | Never |

**Note**: Absolute timing at very low rates (1–4) is faster than real OPL2 hardware due to
`env_level` only being 10-bit (1023 max). A sub-sample accumulator would be needed for
accurate slow-rate timing.

---

## Architecture Notes

### `Operator` helper methods added
- `advance_phase_acc()` — advances phase accumulator by one step (vibrato applied)
- `sample_at_phase(phase, ...)` — samples waveform at given phase without advancing state
  (used by rhythm synthesis where phase is computed externally from bit formulas)

### Rhythm mode operator layout
```
ch 6: bass drum  — mod=slot 12 (OPL_MOD_SLOT[6]), car=slot 15 (OPL_CAR_SLOT[6])
ch 7: hi-hat     — mod=slot 13 (OPL_MOD_SLOT[7])
      snare      — car=slot 16 (OPL_CAR_SLOT[7])
ch 8: tom-tom    — mod=slot 14 (OPL_MOD_SLOT[8])
      cymbal     — car=slot 17 (OPL_CAR_SLOT[8])
```

---

## Known Remaining Issues

1. **Envelope timing at low rates** — rates 1–7 are too fast relative to hardware (need a
   fractional sub-increment accumulator in `Operator`). Practical impact is minor since most
   instruments use attack rates 8+.

2. **Envelope rate table (`env_increment`)** is a rough approximation. Nuked-OPL3 uses a
   full `eg_incstep[4][4]` table combined with key-scale rate adjustment. Current values:
   ```rust
   0=>0, 1=>1, 2=>2, 3=>3, 4=>5, 5=>7, 6=>10, 7=>14,
   8=>20, 9=>28, 10=>40, 11=>56, 12=>80, 13=>112, 14=>160, _=>1023
   ```
   These are approximately 2× too fast per step compared to hardware spec.

3. **Key Scale Rate (KSR)** — `ksr` register field exists but is not applied to envelope rates.
   Higher-pitched notes should have faster envelopes.

4. **Sustain level SL=15** — should be treated as silence threshold (~93 dB), not `15 * 68 = 1020`.
   Currently: `sustain_level = self.sustain as u32 * 68`.
   Should be: `if self.sustain == 15 { 1023 } else { self.sustain as u32 * 68 }` (or similar).

---

## Test Commands

```bash
# Build check
./scripts/pre-commit.sh

# AdLib jukebox
cargo run -p emu86-native-gui -- --sound-card adlib --boot --floppy-a dos.img

# AdLib detection test
cargo run -p emu86-native-gui -- --sound-card adlib test-programs/audio/adlib_detection.com
```

---

## Reference: Nuked-OPL3

Key findings from reading `opl3.c` (nukeykt/Nuked-OPL3):

- **FM depth**: `slot->out` (16-bit, ±8192 max at TL=0) added directly to 10-bit `pg_phase_out`
- **Attack formula**: `eg_rout += ~eg_rout >> (4 - eg_inc)` — exponential approach to full volume
- **Envelope range**: 9-bit (0–511), not 10-bit
- **LFSR**: 23-bit, shift-right, taps at bits 14 and 0
- **Rhythm phase XOR**: specific bits of slots 13 and 17 create metallic sounds without noise amplitude mixing
- **Feedback**: `(prout + out) >> (9 - fb)` — same as our implementation
