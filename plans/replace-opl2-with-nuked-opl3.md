# Plan: Replace hand-rolled OPL2 with Nuked OPL3 Rust Port

## Goal

Replace `core/src/audio/opl2.rs` (hand-rolled OPL2 approximation) with a faithful
Rust port of the Nuked OPL3 emulator (`opl3.c` / `opl3.h`, version 1.8 by Nuke.YKT).

The Nuked emulator is cycle-accurate and uses ROM-extracted log-sine/exp tables from
real hardware, producing significantly better audio fidelity. It runs in OPL2
compatibility mode (`newm = 0`) to preserve AdLib card behaviour.

`opl2.rs` is **deleted**. `adlib.rs` becomes the thin wrapper, owning `Opl3Chip`
directly and handling the timer registers that used to live in `Opl2`.

## Attribution

`nuked_opl3.rs` is a Rust port of **Nuked-OPL3** by Nuke.YKT (version 1.8).

- Source: <https://github.com/nukeykt/Nuked-OPL3>
- License: GNU Lesser General Public License v2.1 (LGPL-2.1)

The file must carry the following header comment:

```rust
// Nuked OPL3 — Rust port of opl3.c / opl3.h (version 1.8)
// Original C implementation by Nuke.YKT
// https://github.com/nukeykt/Nuked-OPL3
//
// This file is licensed under the GNU Lesser General Public License v2.1.
// See <https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html>.
//
// Ported to Rust for emu86 — structural changes only; all algorithms are
// faithful reproductions of the upstream C source.
```

Because LGPL-2.1 is a copyleft license, the `nuked_opl3` module must remain
clearly separable from the rest of the codebase (no inlining its internals into
other modules). Keeping it in its own file satisfies this requirement.

---

## Files

| File | Action | Status |
|------|--------|--------|
| `core/src/audio/nuked_opl3.rs` | **CREATE** — Rust port of `opl3.c` / `opl3.h` | 🔄 In progress |
| `core/src/audio/opl2.rs` | **DELETE** | ⏳ Pending |
| `core/src/audio/adlib.rs` | **UPDATE** — own `Opl3Chip` + timer state directly | ⏳ Pending |
| `core/src/audio/mod.rs` | **UPDATE** — remove `pub mod opl2;`, add `pub mod nuked_opl3;` | ✅ Done (module declared) |

---

## Step 1 — Create `core/src/audio/nuked_opl3.rs`

This is the bulk of the work. Port `opl3.c` to safe Rust, resolving all C raw-pointer
patterns into index-based equivalents.

### 1a. ROM tables (trivial — literal transcription) ✅ DONE

```rust
static LOGSINROM: [u16; 256] = [ /* ... identical to C */ ];
static EXPROM:    [u16; 256] = [ /* ... identical to C */ ];
static MT:        [u8; 16]   = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];
static KSLROM:    [u8; 16]   = [0, 32, 40, 45, 48, 51, 53, 55, 56, 58, 59, 60, 61, 62, 63, 64];
static KSLSHIFT:  [u8; 4]    = [8, 1, 2, 0];
static EG_INCSTEP: [[u8; 4]; 4] = [[0,0,0,0],[1,0,0,0],[1,0,1,0],[1,1,1,0]];
static AD_SLOT:   [i8; 0x20] = [ /* identical to C */ ];
static CH_SLOT:   [u8; 18]   = [0, 1, 2, 6, 7, 8, 12, 13, 14, 18, 19, 20, 24, 25, 26, 30, 31, 32];
```

### 1b. Pointer replacement strategy ✅ DONE

The C code uses raw pointers to wire up the modulation network at channel-setup time.
Rust replaces each with a lightweight enum or index stored in the struct field:

| C pointer field | Rust replacement |
|----------------|-----------------|
| `slot->mod: *i16` | `slot.mod_input: ModInput` (see below) |
| `slot->trem: *u8` | `slot.trem_chip: bool` (true→chip.tremolo, false→0) |
| `channel->slotz[2]: *opl3_slot` | `channel.slotz: [u8; 2]` (slot indices into chip.slots) |
| `channel->out[4]: *i16` | `channel.out: [OutSrc; 4]` (see below) |
| `channel->pair: *opl3_channel` | `channel.pair_idx: u8` (channel index, 0xFF = none) |
| `slot->channel: *opl3_channel` | `slot.channel_num: u8` |
| `chip->zeromod: i16` | constant `0i16`; referenced as `ModInput::Zero` / `OutSrc::Zero` |

```rust
/// Source of a slot's phase-modulation input.
#[derive(Clone, Copy, Default)]
pub enum ModInput {
    #[default]
    Zero,
    SlotOut(u8),   // chip.slots[i].out
    SlotFbMod(u8), // chip.slots[i].fbmod
}

/// Source of a channel's output contribution.
#[derive(Clone, Copy, Default)]
pub enum OutSrc {
    #[default]
    Zero,
    SlotOut(u8),   // chip.slots[i].out
}
```

### 1c. Struct layout ✅ DONE

```rust
pub struct Opl3Slot {
    pub out:      i16,
    pub fbmod:    i16,
    pub mod_input: ModInput,
    pub prout:    i16,
    pub eg_rout:  u16,
    pub eg_out:   u16,
    pub eg_gen:   u8,   // 0=attack,1=decay,2=sustain,3=release
    pub eg_ksl:   u8,
    pub trem_chip: bool,
    pub reg_vib:  u8,
    pub reg_type: u8,
    pub reg_ksr:  u8,
    pub reg_mult: u8,
    pub reg_ksl:  u8,
    pub reg_tl:   u8,
    pub reg_ar:   u8,
    pub reg_dr:   u8,
    pub reg_sl:   u8,
    pub reg_rr:   u8,
    pub reg_wf:   u8,
    pub key:      u8,   // bitfield: egk_norm | egk_drum
    pub pg_reset: bool,
    pub pg_phase: u32,
    pub pg_phase_out: u16,
    pub slot_num: u8,
    pub channel_num: u8,
}

pub struct Opl3Channel {
    pub slotz:    [u8; 2],   // slot indices
    pub pair_idx: u8,        // 0xFF = none
    pub out:      [OutSrc; 4],
    pub chtype:   u8,        // ch_2op / ch_4op / ch_4op2 / ch_drum
    pub f_num:    u16,
    pub block:    u8,
    pub fb:       u8,
    pub con:      u8,
    pub alg:      u8,
    pub ksv:      u8,
    pub cha:      u16,
    pub chb:      u16,
    pub chc:      u16,
    pub chd:      u16,
    pub ch_num:   u8,
}

pub struct Opl3Chip {
    pub channel:  [Opl3Channel; 18],
    pub slot:     [Opl3Slot; 36],
    pub timer:    u16,
    pub eg_timer: u64,
    pub eg_timerrem: u8,
    pub eg_state: u8,
    pub eg_add:   u8,
    pub eg_timer_lo: u8,
    pub newm:     u8,   // Always 0 for OPL2 compat
    pub nts:      u8,
    pub rhy:      u8,
    pub vibpos:   u8,
    pub vibshift: u8,
    pub tremolo:  u8,
    pub tremolopos: u8,
    pub tremoloshift: u8,
    pub noise:    u32,
    pub mixbuff:  [i32; 4],
    pub rm_hh_bit2: u8,
    pub rm_hh_bit3: u8,
    pub rm_hh_bit7: u8,
    pub rm_hh_bit8: u8,
    pub rm_tc_bit3: u8,
    pub rm_tc_bit5: u8,
    // Internal resampler state (OPL3_GenerateResampled)
    pub rateratio:  i32,
    pub samplecnt:  i32,
    pub oldsamples: [i16; 4],
    pub samples:    [i16; 4],
    // Write buffer (OPL_WRITEBUF_SIZE = 1024)
    pub writebuf_samplecnt: u64,
    pub writebuf_cur:  u32,
    pub writebuf_last: u32,
    pub writebuf_lasttime: u64,
    pub writebuf: Vec<Opl3WriteBuf>,  // capacity 1024
}

pub struct Opl3WriteBuf {
    pub time: u64,
    pub reg:  u16,
    pub data: u8,
}
```

### 1d. Methods to port (one-to-one with C functions)

Port each C function as a method or free function. Resolve pointer dereferences
using the `ModInput`/`OutSrc` enums at call sites.

#### Envelope helpers ✅ DONE
- `fn envelope_calc_exp(level: u32) -> i16`
- `fn envelope_calc_sin0..7(phase: u16, envelope: u16) -> i16` (8 waveform functions)
- `pub(crate) fn envelope_calc_sin(wf: u8, phase: u16, envelope: u16) -> i16` (dispatch helper)
- `pub(crate) fn envelope_update_ksl(chip: &mut Opl3Chip, slot_idx: usize)`
- `pub(crate) fn envelope_calc(chip: &mut Opl3Chip, slot_idx: usize)`
  - **Borrow note**: pre-read `slot.channel_num` to fetch `channel.ksv` / `channel.block`,
    pass them as parameters so the exclusive borrow on `chip.slot[slot_idx]` is cleanly split.
- `fn envelope_key_on(slot: &mut Opl3Slot, key_type: u8)`
- `fn envelope_key_off(slot: &mut Opl3Slot, key_type: u8)`

#### Phase generator
- `fn phase_generate(chip: &mut Opl3Chip, slot_idx: usize)`
  - Reads `slot.channel_num` → `chip.channel[ch].f_num` / `.block`
  - Writes `chip.rm_hh_bit*` / `chip.rm_tc_bit*` for rhythm mode
  - Advances `chip.noise`
  - **Borrow note**: snapshot `f_num`, `block`, `reg_vib`, `channel_num` into locals, then
    access `chip.slot[slot_idx]` mutably.

#### Slot operations
- `fn slot_calc_fb(slot: &mut Opl3Slot)`  — pure slot mutation, no aliasing
- `fn slot_generate(chip: &mut Opl3Chip, slot_idx: usize)`
  - Reads mod input value first: `let mod_val = read_mod_input(chip, mod_input);`
  - Reads tremolo: `let trem = if slot.trem_chip { chip.tremolo } else { 0 };`
  - Calls the envelope_sin function for `slot.reg_wf`
  - **Helper**: `fn read_mod_input(chip: &Opl3Chip, mi: ModInput) -> i16`

#### Channel setup
- `fn channel_setup_alg(chip: &mut Opl3Chip, ch_idx: usize)`
  - Translates the C pointer-assignment pattern into setting `slot.mod_input` and
    `channel.out[i]` enum values
- `fn channel_update_alg(chip: &mut Opl3Chip, ch_idx: usize)`
- `fn channel_update_rhythm(chip: &mut Opl3Chip, data: u8)`

#### Per-channel write handlers
- `fn channel_write_a0(chip: &mut Opl3Chip, ch_idx: usize, data: u8)`
- `fn channel_write_b0(chip: &mut Opl3Chip, ch_idx: usize, data: u8)`
- `fn channel_write_c0(chip: &mut Opl3Chip, ch_idx: usize, data: u8)`
- `fn channel_key_on(chip: &mut Opl3Chip, ch_idx: usize)`
- `fn channel_key_off(chip: &mut Opl3Chip, ch_idx: usize)`

#### Per-slot write handlers
- `fn slot_write_20/40/60/80/e0(slot: &mut Opl3Slot, data: u8)` (slot borrows only, no aliasing)
  - `slot_write_40` calls `envelope_update_ksl(chip, slot_idx)` — needs chip + idx

#### Top-level generation
- `pub fn process_slot(chip: &mut Opl3Chip, slot_idx: usize)`
  - Calls: `slot_calc_fb`, `envelope_calc`, `phase_generate`, `slot_generate` in order
- `pub fn generate_4ch(chip: &mut Opl3Chip, buf4: &mut [i16; 4])`
  - Port `OPL3_Generate4Ch` exactly, including `OPL_QUIRK_CHANNELSAMPLEDELAY = 1`
  - Channel mix: `fn channel_accm(chip: &Opl3Chip, ch: usize) -> i16` — reads all out[i]
  - Apply `cha`/`chb`/`chc`/`chd` masking for stereo (OPL2: cha=0xFFFF, chc=0)
- `pub fn generate_resampled(chip: &mut Opl3Chip, buf: &mut [i16; 2])`
  - Port `OPL3_GenerateResampled` exactly (fixed-point linear interpolation)

#### Public API
- `pub fn reset(chip: &mut Opl3Chip, samplerate: u32)`
  - `memset` → `*chip = Opl3Chip::default()` then explicit init
  - `chip.rateratio = (samplerate as i32 * (1 << RSM_FRAC)) / 49716`
  - Wire up slotz, pair_idx, channel_num, ch_slot, OPL3_ChannelSetupAlg for all channels
  - **OPL2 compat**: `chip.newm = 0` (never set to 1)
- `pub fn write_reg(chip: &mut Opl3Chip, reg: u16, v: u8)`
  - Port `OPL3_WriteReg` — dispatch on `(reg >> 4) & 0xF` then call slot/channel writers
  - For OPL2 mode: `high = 0` always; limit waveforms to 0-3
- `pub fn write_reg_buffered(chip: &mut Opl3Chip, reg: u16, v: u8)`
  - Port `OPL3_WriteRegBuffered`

### 1e. Borrow checker patterns

The two recurring borrow challenges in porting `generate_4ch`:

**Pattern 1 — read mod value before mutating slot:**
```rust
// Read mod input (immutable borrow of chip ends here)
let mod_val = match chip.slot[slot_idx].mod_input {
    ModInput::Zero => 0i16,
    ModInput::SlotOut(i)   => chip.slot[i as usize].out,
    ModInput::SlotFbMod(i) => chip.slot[i as usize].fbmod,
};
// Now mutate the slot (exclusive borrow OK)
slot_generate_with_mod(&mut chip.slot[slot_idx], mod_val, trem_val);
```

**Pattern 2 — read channel fields before mutating slot:**
```rust
let ch_num    = chip.slot[slot_idx].channel_num as usize;
let f_num     = chip.channel[ch_num].f_num;
let block     = chip.channel[ch_num].block;
let ksv       = chip.channel[ch_num].ksv;
// ... now safely mutate slot[slot_idx]
```

---

## Step 2 — Update `core/src/audio/adlib.rs`

`adlib.rs` becomes the thin wrapper. Delete the `use crate::audio::opl2::Opl2;` import
and replace the `opl2: Opl2` field with `Opl3Chip` plus the timer state that used to
live in `Opl2`.

### New `Adlib` struct fields

```rust
use crate::audio::nuked_opl3::{self, Opl3Chip};

pub struct Adlib {
    chip: Opl3Chip,
    // Timer state (moved here from Opl2)
    pending_address: u8,
    timer1_value: u8,
    timer2_value: u8,
    timer_control: u8,
    timer1_counter: u32,
    timer2_counter: u32,
    pub status: u8,
    // CPU-cycles → output-samples accumulator
    cycle_acc: u64,
    cpu_freq: u64,
    timer1_cycles_per_tick: u32,
    timer2_cycles_per_tick: u32,
    // Ring buffer and scratch (unchanged)
    consumer: PcmRingBuffer,
    samples_scratch: Vec<f32>,
    pending_flush: Vec<f32>,
    overflow_count: u64,
    samples_since_log: u64,
}
```

### Initialization

```rust
pub fn new(cpu_freq: u64) -> Self {
    let mut chip = Opl3Chip::default();
    nuked_opl3::reset(&mut chip, ADLIB_SAMPLE_RATE);
    let timer1_cycles_per_tick = (80e-6 * cpu_freq as f64).round() as u32;
    let timer2_cycles_per_tick = (320e-6 * cpu_freq as f64).round() as u32;
    Self {
        chip,
        pending_address: 0,
        timer1_value: 0,
        timer2_value: 0,
        timer_control: 0,
        timer1_counter: 0,
        timer2_counter: 0,
        status: 0,
        cycle_acc: 0,
        cpu_freq,
        timer1_cycles_per_tick,
        timer2_cycles_per_tick,
        consumer: PcmRingBuffer::new(DEFAULT_CAPACITY),
        samples_scratch: Vec::new(),
        pending_flush: Vec::with_capacity(FLUSH_SIZE * 2),
        overflow_count: 0,
        samples_since_log: 0,
    }
}
```

### `SoundCard` port write handler

Timer registers 0x02, 0x03, 0x04 are intercepted in `adlib.rs`;
all other writes pass through to `nuked_opl3::write_reg`:

```rust
fn write_port(&mut self, port: u16, value: u8) {
    match port {
        0x388 => self.pending_address = value,
        0x389 => {
            let addr = self.pending_address;
            match addr {
                0x02 => self.timer1_value = value,
                0x03 => self.timer2_value = value,
                0x04 => self.handle_timer_control(value),
                _ => nuked_opl3::write_reg(&mut self.chip, addr as u16, value),
            }
        }
        _ => {}
    }
}

fn read_port(&mut self, port: u16) -> u8 {
    match port {
        0x388 | 0x389 => self.status,
        _ => 0xFF,
    }
}
```

### Timer methods (copied verbatim from old `Opl2`)

Move `handle_timer_control` and `advance_timers` from `opl2.rs` into `adlib.rs` as
private `impl Adlib` methods. No logic changes.

### Sample generation in `tick()`

```rust
fn tick(&mut self, cpu_cycles: u64) {
    self.advance_timers(cpu_cycles);

    self.cycle_acc += cpu_cycles * ADLIB_SAMPLE_RATE as u64;
    let n_out = self.cycle_acc / self.cpu_freq;
    self.cycle_acc %= self.cpu_freq;

    for _ in 0..n_out {
        let mut buf = [0i16; 2];
        nuked_opl3::generate_resampled(&mut self.chip, &mut buf);
        let mono = (buf[0] as i32 + buf[1] as i32) / 2;
        self.pending_flush.push(mono as f32 / 32768.0);
    }

    if self.pending_flush.len() >= FLUSH_SIZE {
        self.flush_pending();
    }
}
```

### Reset

```rust
fn reset(&mut self) {
    self.pending_flush.clear();
    nuked_opl3::reset(&mut self.chip, ADLIB_SAMPLE_RATE);
    self.pending_address = 0;
    self.timer1_value = 0;
    self.timer2_value = 0;
    self.timer_control = 0;
    self.timer1_counter = 0;
    self.timer2_counter = 0;
    self.status = 0;
    self.cycle_acc = 0;
    self.consumer.inner.lock().unwrap().clear();
}
```

---

## Step 3 — Update `core/src/audio/mod.rs`

Replace `pub mod opl2;` with `pub mod nuked_opl3;`:

```rust
pub mod nuked_opl3;
pub mod adlib;
pub mod speaker;
```

Then delete `core/src/audio/opl2.rs`.

---

## Step 4 — Run `./scripts/pre-commit.sh`

Fix any compiler errors or Clippy warnings before declaring done.

---

## Key correctness notes

### OPL2 vs OPL3 compat
- `chip.newm = 0` (set in `reset`, never changed by `write_reg` since `reg 0x105` is
  second bank and we only use first bank in OPL2 mode)
- Waveforms clamped to 0-3 inside `slot_write_e0` when `newm == 0`

### Timer protocol (AdLib detection)
The original `handle_timer_control` / `advance_timers` logic is preserved exactly.
Programs detect AdLib by:
1. Writing known values to timer registers 0x02/0x03
2. Starting timers via reg 0x04
3. Reading status 0x388 and checking bits 6/5 for overflow

### `OPL_QUIRK_CHANNELSAMPLEDELAY = 1` (default)
The left/right channel sample interleaving pattern in `generate_4ch` must match the
C code precisely. Slots 0-14 process before left mix; slots 15-17 after; slots 18-32
before right mix; slots 33-35 after. This is the default path in the C code.

### Tremolo / vibrato accuracy
The Nuked OPL3 tremolo is a triangle wave (`tremolopos` counter at 1/64th of OPL
rate, triangle up/down over 210 positions with `tremoloshift` for depth). Vibrato
is 8-step LUT indexed by `vibpos`. Both are significantly more accurate than the
sine-based LFOs in the original `opl2.rs`.

### Sustain level 15 expansion
`slot_write_80`: when `reg_sl == 0x0f`, expand to `0x1f` (matches C exactly).

---

## Future: OPL3 support for other sound cards

The Nuked OPL3 chip can run in full OPL3 mode (`chip.newm = 1`) for cards that
support it (e.g. Sound Blaster Pro 2 / OPL3, Sound Blaster 16). To enable this
later, without touching the OPL2/AdLib path:

- Add a `SoundCardType::SoundBlaster16` (or similar) variant
- Create a new `core/src/audio/sb16.rs` (analogous to `adlib.rs`) that owns its
  own `Opl3Chip` and calls `nuked_opl3::reset(&mut chip, samplerate)` with
  `chip.newm = 1` after reset
- The OPL3 register bank (port `0x222`/`0x223`, or `0x388`/`0x389` second bank via
  `reg | 0x100`) is handled inside `nuked_opl3::write_reg` when `newm == 1`; no
  changes to `nuked_opl3.rs` are needed
- Stereo output: `generate_resampled` already fills `buf[0]` (left) and `buf[1]`
  (right) separately; the SB16 wrapper can push them to separate channels instead
  of mono-mixing

The key design constraint is: **do not add `newm`-toggle logic to `adlib.rs`**.
Keep `adlib.rs` permanently in OPL2 mode; OPL3 features live in a separate file.

---

## Estimated effort

| Section | Complexity |
|---------|-----------|
| ROM tables | Trivial copy |
| Enum types + struct layout | Low |
| Envelope helpers | Low |
| Phase generator | Medium (noise, rhythm bits) |
| Slot write handlers | Low |
| `slot_calc_fb`, `slot_generate` | Medium (borrow patterns) |
| `channel_setup_alg` | High (pointer-to-enum translation) |
| `generate_4ch` | High (ordering, mixbuff, borrow patterns) |
| `generate_resampled` | Low (fixed-point arithmetic) |
| `reset` | Medium (wiring slotz/pair_idx/ch_num) |
| `write_reg` dispatch | Medium |
| `adlib.rs` update | Low |

Total: 1–2 focused sessions.
