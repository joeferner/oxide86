# AdLib Sound Card Support Plan

## Overview

Add AdLib (Yamaha OPL2/YM3812) FM synthesis sound card emulation. Rename
`--no-audio` to `--disable-pc-speaker`. Add a `--sound-card` flag to select
which sound card to emulate. Design the architecture to accommodate future
additions (Sound Blaster, etc.).

---

## Phase 1: CLI Changes

### 1.1 Rename `--no-audio` → `--disable-pc-speaker`

**File**: `native-common/src/cli.rs`

- Change `#[arg(long = "no-audio")]` to `#[arg(long = "disable-pc-speaker")]`
- Rename field `no_audio` → `disable_pc_speaker`
- Update all call sites: `native-cli/src/main.rs`, `native-gui/src/main.rs`

### 1.2 Add `--sound-card` Option

**File**: `native-common/src/cli.rs`

```rust
/// Sound card to emulate (none, adlib)
#[arg(long = "sound-card", default_value = "none")]
pub sound_card: String,
```

Valid values for now: `none`, `adlib`. Future: `sb` (Sound Blaster).

**WASM** (`wasm/src/lib.rs` `ComputerConfig`):

```rust
/// Sound card to emulate ("none", "adlib")
pub sound_card: Option<String>,
```

---

## Phase 2: Core Sound Module

### 2.1 Directory Structure

```
core/src/sound/
├── mod.rs      - SoundCard trait, SoundCardType enum, NullSoundCard
└── adlib.rs    - AdLib OPL2 register state machine
```

### 2.2 `SoundCard` Trait (`core/src/sound/mod.rs`)

```rust
/// Platform-independent sound card trait.
/// The card handles I/O port reads/writes and produces PCM samples.
pub trait SoundCard: Send {
    /// Handle a port write (address or data register).
    fn write_port(&mut self, port: u16, value: u8);
    /// Handle a port read (e.g., status register).
    fn read_port(&mut self, port: u16) -> u8;
    /// Generate `num_samples` interleaved stereo f32 samples at the given sample rate.
    fn generate_samples(&mut self, num_samples: usize, sample_rate: u32) -> Vec<f32>;
}

pub struct NullSoundCard;
impl SoundCard for NullSoundCard { /* no-ops */ }

pub enum SoundCardType { None, AdLib }

impl SoundCardType {
    pub fn from_str(s: &str) -> Self { ... }
}
```

### 2.3 AdLib State Machine (`core/src/sound/adlib.rs`)

The AdLib card exposes two I/O ports:
- `0x388` – address register (write) / status register (read)
- `0x389` – data register (write)

**Status register** (read from 0x388):
- Bit 7: Timer 1 or Timer 2 overflow
- Bit 6: Timer 1 overflow
- Bit 5: Timer 2 overflow

The `AdLib` struct wraps an `Opl2` engine (see Phase 3):

```rust
pub struct AdLib {
    address: u8,
    opl2: Opl2,
    // Timers (for proper status byte emulation)
    timer1: u8,
    timer2: u8,
    timer_control: u8,
}

impl SoundCard for AdLib {
    fn write_port(&mut self, port: u16, value: u8) {
        match port {
            0x388 => self.address = value,
            0x389 => self.opl2.write(self.address, value),
            _ => {}
        }
    }
    fn read_port(&mut self, port: u16) -> u8 {
        match port {
            0x388 => self.opl2.status(),
            _ => 0xFF
        }
    }
    fn generate_samples(&mut self, num_samples: usize, sample_rate: u32) -> Vec<f32> {
        self.opl2.generate(num_samples, sample_rate)
    }
}
```

**Port detection detection sequence** (used by AdLib games to detect card):
1. Write 0x60 to register 0x04 (reset timers)
2. Write 0x80 to register 0x04 (enable display of flags)
3. Read status – expect 0x00
4. Write 0xFF to register 0x02 (set timer 1)
5. Write 0x21 to register 0x04 (start timer 1)
6. Short delay
7. Read status – expect bit 7 and bit 6 set (0xC0)

The emulator should pass this detection sequence by tracking timer state in
`AdLib` and returning appropriate status bytes.

---

## Phase 3: OPL2 FM Synthesis Engine

### 3.1 Crate Evaluation

Before implementing from scratch, search crates.io for a suitable Rust OPL2
crate. Candidates to evaluate:

- `opl` – if it provides OPL2 synthesis and generates PCM samples
- `ymfm-rs` or similar YM3812 bindings
- Any crate wrapping nuked-opl3 (overkill, but well-tested)

**Criteria**: must support OPL2 register writes, PCM generation at 44100/48000
Hz, work in both native and WASM (no OS-specific dependencies in the crate
itself), and be actively maintained.

### 3.2 If No Suitable Crate Exists: Implement `Opl2`

**File**: `core/src/sound/opl2.rs`

OPL2 (YM3812) features to implement:
- 9 FM channels, each with 2 operators (Modulator + Carrier)
- Per-operator registers: AVEKM (attack, decay, sustain, release, waveform, etc.)
- Per-channel registers: frequency number, block, key-on, algorithm (AM/FM)
- Global registers: AM/VIB depth, rhythm mode, waveform select enable
- Four waveforms: sine, half-sine, absolute sine, quarter-sine

**Implementation approach** (based on public-domain OPL2 references):

```
Opl2
├── operators[18]: Operator    (2 per channel × 9 channels)
├── channels[9]:  Channel
├── global:       GlobalOpl2State
└── sample_counter: f64        (fractional sample accumulator)
```

Key implementation notes:
- OPL2 internal clock: 3.579545 MHz / 72 = ~49,716 Hz internal sample rate
- Upsample/downsample to target sample rate (44100 or 48000 Hz)
- Envelope generator: 4-stage ADSR with 64-step exponential curves
- Frequency: `Fnum * 2^(Block-1) * 3.579545 MHz / 2^20`
- FM feedback: channel operator 1 can feedback into itself (3 bits)

Reference implementations for correctness:
- DOSBox's `dbopl.cpp` (LGPL)
- MAME's `fm.cpp`
- Nuked-OPL3 (bit-accurate but OPL3)

---

## Phase 4: Native Audio Output

### 4.1 Approach

Keep PC speaker on its own Rodio sink. Add AdLib on a second Rodio sink.
Both mix at the OS/driver level. This avoids complex in-process mixing and
reuses the existing clean speaker architecture.

**File**: `native-common/src/setup.rs` – add:

```rust
pub fn create_sound_card(
    card_type: SoundCardType,
    sample_rate: u32,
) -> (Box<dyn SoundCard>, Option<AdLibOutput>) {
    match card_type {
        SoundCardType::None => (Box::new(NullSoundCard), None),
        SoundCardType::AdLib => {
            let adlib = AdLib::new();
            let output = AdLibOutput::new(sample_rate); // Rodio-based
            (Box::new(adlib), Some(output))
        }
    }
}
```

**File**: `native-common/src/adlib_output.rs` (new):

```rust
/// Pulls PCM samples from AdLib card and streams them via Rodio.
pub struct AdLibOutput {
    _stream: OutputStream,
    sink: Sink,
    sample_sender: SyncSender<Vec<f32>>,
    sample_rate: u32,
}
```

Use a `rodio::Source` wrapping a ring-buffer fed by the emulator loop. The
`AdLibOutput::push_samples(samples: Vec<f32>)` method sends samples to the
Rodio thread.

### 4.2 Sample Generation in Emulator Loop

**File**: `core/src/computer.rs`

Add alongside `speaker_update_cycles`:

```rust
sound_card: Box<dyn SoundCard>,
sound_card_update_cycles: u64,
```

In `increment_cycles()`:

```rust
self.sound_card_update_cycles += cycles;
// ~100 cycles ≈ every 21µs at 4.77 MHz
// Enough to maintain low latency
if self.sound_card_update_cycles >= SOUND_CARD_UPDATE_INTERVAL {
    self.sound_card_update_cycles = 0;
    self.update_sound_card();
}
```

`update_sound_card()` generates a batch of samples and enqueues them to the
platform output. Since `SoundCard` lives in `core`, sample generation happens
in core. The platform output is accessed via a callback/channel.

**Alternative (simpler)**: Expose `sound_card.generate_samples()` and call
it from the platform layer via `computer.get_sound_card_mut()`.

### 4.3 Callback Approach for Platform Audio Push

To avoid platform dependencies in core:
- Add `set_sound_card_sample_callback(cb: Box<dyn FnMut(Vec<f32>) + Send>)` to `Computer`
- Native code registers a closure that sends to the Rodio ring buffer
- WASM code registers a closure that calls a JS AudioWorklet port

This keeps core platform-neutral.

---

## Phase 5: WASM Audio Output

**File**: `wasm/src/web_adlib.rs` (new)

Use the Web Audio API `ScriptProcessorNode` (deprecated but universally
supported) or `AudioWorkletNode` (modern) to consume PCM samples from the
emulator loop.

**Recommended approach**: `ScriptProcessorNode` for simplicity (AdLib
bandwidth is low: ~48 kHz × 2ch × f32 = ~384 kB/s, easily handled).

```rust
pub struct WebAdLib {
    audio_context: AudioContext,
    processor: ScriptProcessorNode,
    sample_buffer: Arc<Mutex<VecDeque<f32>>>,
}
```

The `onaudioprocess` callback drains samples from the buffer and copies to
the output buffer. Samples are pushed via `push_samples()` from the WASM main
loop.

**Feature flags in `wasm/Cargo.toml`**:

```toml
[dependencies.web-sys]
features = [
    # existing...
    "ScriptProcessorNode",
    "AudioProcessingEvent",
    "AudioBuffer",
]
```

---

## Phase 6: I/O Port Routing

**File**: `core/src/io/mod.rs`

Add AdLib port handling. AdLib primary ports: `0x388`–`0x389`.
Sound Blaster also uses `0x220`–`0x22F` (for future SB support, leave room).

In `IoDevice::read_port()` / `write_port()`:

```rust
// Sound card ports – routed through SoundCard trait
0x388..=0x389 => { ... }
```

Since `SoundCard` is owned by `Computer` (not `IoDevice`), one option:
- Store port reads/writes in `IoDevice` as a pending queue
- `Computer::step()` drains the queue and forwards to `sound_card`

Or simpler:
- Give `IoDevice` a `Box<dyn SoundCard>` (same ownership model as `Speaker`)
- Forward directly in `IoDevice::read_port()` / `write_port()`

**Recommended**: Move `sound_card` into `IoDevice` for direct port dispatch,
matching how other I/O devices (PIT, keyboard controller) are handled.

---

## Phase 7: Sound Blaster Compatibility Stub

When `--sound-card adlib` is active, also respond to `0x388`/`0x389` (pure
AdLib). For future `--sound-card sb`:

- Sound Blaster (SB 1.0): OPL2 at `0x388` + DSP at `0x22x` + IRQ/DMA
- Sound Blaster Pro: OPL2 stereo at `0x220`
- Sound Blaster 16: OPL3 at `0x220`

Design `SoundCard` trait to be extensible:
- `fn port_range(&self) -> &[(u16, u16)]` – list of (start, end) port ranges
  the card handles
- `IoDevice` iterates registered cards and routes accordingly

This allows multiple virtual cards simultaneously if needed.

---

## File Changes Summary

| File | Change |
|------|--------|
| `native-common/src/cli.rs` | Rename `no_audio` → `disable_pc_speaker`; add `sound_card: String` |
| `native-common/src/setup.rs` | Add `create_sound_card()` |
| `native-common/src/adlib_output.rs` | **New** – Rodio PCM stream for AdLib |
| `native-cli/src/main.rs` | Pass `--disable-pc-speaker`, `--sound-card` to setup |
| `native-gui/src/main.rs` | Same |
| `core/src/sound/mod.rs` | **New** – `SoundCard` trait, `NullSoundCard`, `SoundCardType` |
| `core/src/sound/adlib.rs` | **New** – `AdLib` struct, port dispatch, timer emulation |
| `core/src/sound/opl2.rs` | **New** (if no crate) – OPL2 FM synthesis |
| `core/src/io/mod.rs` | Route ports `0x388`/`0x389` to sound card |
| `core/src/computer.rs` | Add `sound_card` field, `update_sound_card()`, sample callback |
| `wasm/src/lib.rs` | Add `sound_card` to `ComputerConfig` |
| `wasm/src/web_adlib.rs` | **New** – Web Audio API consumer for AdLib samples |
| `wasm/www/pkg/emu86_wasm.d.ts` | Update TS types for new config field |
| `CLAUDE.md` | Document new `--sound-card` option and AdLib ports |

---

## Implementation Order

1. **Phase 1** (CLI changes) – purely additive, low risk
2. **Phase 2** (core sound trait + AdLib state) – no audio output yet, just I/O
3. **Phase 3** (OPL2 engine) – most complex; evaluate crates first
4. **Phase 6** (I/O port routing) – wire ports into the emulator
5. **Phase 4** (native audio output) – Rodio ring buffer
6. **Phase 5** (WASM output) – ScriptProcessorNode
7. **Phase 7** (SB stub) – document port ranges for future work

---

## Testing

Create `test-programs/sound/adlib_detect.asm`:
- Run the standard AdLib detection sequence
- Write a known register value, read it back
- Confirm status byte after timer test

Create `test-programs/sound/adlib_tone.asm`:
- Set up a single FM channel
- Play a 440 Hz tone for ~2 seconds
- Silence

Update `test-programs/README.md` with both.
