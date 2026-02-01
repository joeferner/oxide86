# PC Speaker Emulation Implementation Plan

## Overview
Implement accurate PC speaker emulation with full PIT (Programmable Interval Timer) Channel 2 support for both native and WASM platforms.

## Architecture Approach

**Full PIT Emulation**: Implement Intel 8253/8254 PIT with all 3 channels (Channel 2 for speaker)
**Trait-Based Abstraction**: `SpeakerOutput` trait for platform independence (follows `KeyboardInput`/`VideoController` patterns)
**Native Audio**: Rodio library with infinite square wave generator
**WASM Ready**: Architecture supports future Web Audio API implementation
**Always Enabled**: Audio compiled in by default, graceful fallback to `NullSpeaker` if unavailable

## Implementation Phases

### Phase 1: Core PIT Emulation

**File**: `core/src/pit.rs` (CREATE)

Implement Intel 8253/8254 PIT with 3 channels:
- **Channel 0**: System timer (18.2 Hz) - note existing via BDA, PIT adds programmability
- **Channel 1**: Memory refresh (stub only, not needed for emulation)
- **Channel 2**: Speaker control (primary focus)

**Key structures**:
```rust
pub struct PitChannel {
    count_register: u16,     // Reload value
    counter: u16,            // Current count
    output: bool,            // Output state
    mode: u8,                // Mode 0-5 (focus on Mode 3: square wave)
    access_mode: u8,         // LSB/MSB/Both
    latch_value: Option<u16>,
    gate: bool,              // Gate input (port 0x61 bit 0 for channel 2)
    write_lsb_next: bool,
    read_lsb_next: bool,
}

pub struct Pit {
    channels: [PitChannel; 3],
}
```

**Critical methods**:
- `write_channel(channel: u8, value: u8)` - Write to ports 0x40-0x42
- `read_channel(channel: u8) -> u8` - Read from ports 0x40-0x42
- `write_command(command: u8)` - Configure via port 0x43
- `update(cycles: u64)` - Update counters, called from `Computer::increment_cycles()`
- `get_channel_output(channel: u8) -> bool` - Get Timer 2 output for port 0x61 bit 5
- `set_gate(channel: u8, gate: bool)` - Set gate from port 0x61 bit 0

**PIT Specifications**:
- Base frequency: 1.193182 MHz (1193182 Hz)
- Output frequency = 1193182 / count_register
- Mode 3 (square wave): Most common for PC speaker

### Phase 2: Speaker Trait Definition

**File**: `core/src/speaker.rs` (CREATE)

Define trait for platform-independent speaker output:

```rust
pub trait SpeakerOutput: Send {
    fn set_frequency(&mut self, enabled: bool, frequency: f32);
    fn update(&mut self);
}

pub struct NullSpeaker;
impl SpeakerOutput for NullSpeaker {
    fn set_frequency(&mut self, _enabled: bool, _frequency: f32) {}
    fn update(&mut self) {}
}
```

**Export from** `core/src/lib.rs`:
```rust
pub use speaker::{SpeakerOutput, NullSpeaker};
```

### Phase 3: I/O Port Integration

**File**: `core/src/io/mod.rs` (MODIFY)

Add PIT instance and route ports 0x40-0x43:

```rust
pub struct IoDevice {
    last_write: HashMap<u16, u8>,
    system_control_port: SystemControlPort,
    pit: Pit,  // ADD THIS
}

// In read_byte():
match port {
    0x40..=0x42 => self.pit.read_channel((port - 0x40) as u8),
    0x43 => 0xFF, // Command port is write-only
    0x61 => {
        let mut value = self.system_control_port.read();
        // Set bit 5 (Timer 2 output) from PIT
        if self.pit.get_channel_output(2) {
            value |= 0x20;
        }
        value
    }
    // ... rest
}

// In write_byte():
match port {
    0x40..=0x42 => self.pit.write_channel((port - 0x40) as u8, value),
    0x43 => self.pit.write_command(value),
    0x61 => {
        self.system_control_port.write(value);
        // Update PIT Channel 2 gate from bit 0
        self.pit.set_gate(2, (value & 0x01) != 0);
    }
    // ... rest
}
```

**File**: `core/src/io/system_control_port.rs` (MODIFY)

Add getter for control bits:
```rust
pub fn get_control_bits(&self) -> u8 {
    self.control_bits
}
```

### Phase 4: Computer Integration

**File**: `core/src/computer.rs` (MODIFY)

1. Add `SpeakerOutput` generic parameter:
```rust
pub struct Computer<K: KeyboardInput, V: VideoController = NullVideoController, S: SpeakerOutput = NullSpeaker> {
    // ... existing fields ...
    speaker: S,
}
```

2. Update constructor:
```rust
pub fn new(keyboard: K, mouse: Box<dyn MouseInput>, video_controller: V, speaker: S) -> Self
```

3. Add speaker update to `increment_cycles()`:
```rust
fn increment_cycles(&mut self, cycles: u64) {
    // ... existing timer tick code ...

    // Update PIT (delegates to IoDevice which owns PIT)
    self.io_device.pit.update(cycles);

    // Update speaker based on PIT state
    self.update_speaker();
}

fn update_speaker(&mut self) {
    let control_bits = self.io_device.system_control_port.get_control_bits();
    let timer2_gate = (control_bits & 0x01) != 0;
    let speaker_data = (control_bits & 0x02) != 0;

    // Speaker enabled when both gate and data bits set
    let enabled = timer2_gate && speaker_data;

    if enabled && self.io_device.pit.channels[2].count_register > 0 {
        let count = self.io_device.pit.channels[2].count_register;
        let frequency = 1193182.0 / (count as f32);
        self.speaker.set_frequency(true, frequency);
    } else {
        self.speaker.set_frequency(false, 0.0);
    }
}
```

4. Add public getter for periodic updates:
```rust
pub fn update_speaker_output(&mut self) {
    self.speaker.update();
}
```

### Phase 5: Native Rodio Implementation

**File**: `native/src/rodio_speaker.rs` (CREATE)

Implement speaker using Rodio with infinite square wave source:

```rust
use emu86_core::speaker::SpeakerOutput;
use rodio::{OutputStream, Sink, Source};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct SquareWave {
    frequency: Arc<Mutex<f32>>,
    sample_rate: u32,
    phase: f32,
}

impl Iterator for SquareWave {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let freq = *self.frequency.lock().unwrap();
        if freq <= 0.0 {
            return Some(0.0);
        }
        let phase_increment = freq / (self.sample_rate as f32);
        self.phase = (self.phase + phase_increment) % 1.0;
        Some(if self.phase < 0.5 { 0.3 } else { -0.3 }) // 30% volume
    }
}

impl Source for SquareWave {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 1 }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<Duration> { None }
}

pub struct RodioSpeaker {
    _stream: OutputStream,
    sink: Sink,
    frequency: Arc<Mutex<f32>>,
    enabled: bool,
}

impl RodioSpeaker {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| format!("Audio device unavailable: {}", e))?;
        let sink = Sink::try_new(&handle)
            .map_err(|e| format!("Failed to create audio sink: {}", e))?;

        let frequency = Arc::new(Mutex::new(0.0));
        let wave = SquareWave {
            frequency: frequency.clone(),
            sample_rate: 48000,
            phase: 0.0,
        };

        sink.append(wave);
        sink.pause();

        Ok(Self { _stream: stream, sink, frequency, enabled: false })
    }
}

impl SpeakerOutput for RodioSpeaker {
    fn set_frequency(&mut self, enabled: bool, frequency: f32) {
        *self.frequency.lock().unwrap() = frequency;
        if enabled != self.enabled {
            if enabled { self.sink.play(); } else { sink.pause(); }
            self.enabled = enabled;
        }
    }

    fn update(&mut self) {
        // Rodio handles buffering automatically
    }
}
```

**File**: `native/Cargo.toml` (MODIFY)

Add dependency:
```toml
rodio = "0.19"
```

### Phase 6: Binary Integration

**File**: `native/src/main.rs` (MODIFY)

1. Import speaker module:
```rust
mod rodio_speaker;
use rodio_speaker::RodioSpeaker;
```

2. Instantiate speaker in `main()`:
```rust
let keyboard = TerminalKeyboard::new();
let mouse = Box::new(terminal_mouse.clone_shared());
let video = TerminalVideo::new();

// Create speaker with fallback
let speaker = RodioSpeaker::new()
    .map_err(|e| {
        log::warn!("PC speaker unavailable: {}", e);
        e
    })
    .unwrap_or_else(|_| {
        log::info!("Using NullSpeaker (no audio)");
        // Use NullSpeaker - but type mismatch! Need Box<dyn>
    });

let mut computer = Computer::new(keyboard, mouse, video, speaker);
```

**NOTE**: Since we're using concrete types, we need to handle the fallback case. Two options:
- **Option A**: Use `Box<dyn SpeakerOutput>` (like mouse) for runtime flexibility
- **Option B**: Use a wrapper enum or conditional compilation

Recommend **Option A** for simplicity - change `Computer` to use `Box<dyn SpeakerOutput>` similar to mouse.

**File**: `native-gui/src/main.rs` (MODIFY)
Same changes as native binary.

### Phase 7: WASM Preparation

**File**: `core/src/speaker.rs` (already done with trait)

The trait-based design is already WASM-ready. Future WASM implementation would create:

**File**: `wasm/src/web_speaker.rs` (FUTURE)
```rust
pub struct WebSpeaker {
    audio_context: web_sys::AudioContext,
    oscillator: Option<web_sys::OscillatorNode>,
    gain: web_sys::GainNode,
}
// Implementation uses Web Audio API with OscillatorNode type="square"
```

**Note**: For full WASM implementation details including keyboard, video, and disk management, see [plans/02-wasm.md](wasm.md).

## Critical Files Summary

| File | Action | Purpose |
|------|--------|---------|
| `core/src/pit.rs` | CREATE | PIT 8253/8254 emulation with 3 channels |
| `core/src/speaker.rs` | CREATE | SpeakerOutput trait + NullSpeaker |
| `core/src/io/mod.rs` | MODIFY | Route ports 0x40-0x43, integrate PIT, update port 0x61 bit 5 |
| `core/src/io/system_control_port.rs` | MODIFY | Add `get_control_bits()` accessor |
| `core/src/computer.rs` | MODIFY | Add SpeakerOutput generic, integrate PIT updates, speaker frequency calculation |
| `core/src/lib.rs` | MODIFY | Export Pit, SpeakerOutput, NullSpeaker |
| `native/src/rodio_speaker.rs` | CREATE | Rodio-based speaker with square wave generation |
| `native/Cargo.toml` | MODIFY | Add rodio = "0.19" dependency |
| `native/src/main.rs` | MODIFY | Instantiate RodioSpeaker, pass to Computer |
| `native-gui/src/main.rs` | MODIFY | Same as native/main.rs |

## Verification Strategy

Since we avoid writing tests, verify manually:

### Test 1: Basic Beep (1000 Hz)
```nasm
; beep.asm - Compile with: nasm -f bin beep.asm -o beep.com
org 0x100

; Set PIT Channel 2 to Mode 3 (square wave), 1000 Hz
mov al, 0xB6        ; Channel 2, LSB+MSB, Mode 3, Binary
out 0x43, al
mov ax, 1193        ; Divisor for ~1000 Hz (1193182 / 1193 ≈ 1000)
out 0x42, al        ; LSB
mov al, ah
out 0x42, al        ; MSB

; Enable speaker (set bits 0 and 1 of port 0x61)
in al, 0x61
or al, 0x03
out 0x61, al

; Wait ~1 second
mov cx, 0xFFFF
.delay:
    loop .delay

; Disable speaker
in al, 0x61
and al, 0xFC
out 0x61, al

; Exit
mov ah, 0x4C
int 0x21
```

**Expected**: Hear 1-second 1000 Hz tone

### Test 2: Frequency Sweep
Create program that sweeps from 500 Hz to 2000 Hz over 3 seconds.

### Test 3: DOS Programs
- **QBASIC**: `BEEP` command
- **DOS EDIT**: Error beeps (should work if EDIT uses INT 21h or direct port access)
- **DOS Games**: Any game with PC speaker sound effects

### Test 4: Logging
Add debug logging in `core/src/io/mod.rs` to verify:
- PIT command writes to port 0x43
- Count register writes to port 0x42
- Port 0x61 writes enabling speaker
- Calculated frequencies match expected values

Check `emu86.log` for messages like:
```
I/O Write: Port 0x0043 <- 0xB6
I/O Write: Port 0x0042 <- 0xA9
I/O Write: Port 0x0042 <- 0x04
Speaker: Frequency set to 1000.15 Hz (count=1193)
```

## Implementation Notes

### PIT Mode 3 Details
Mode 3 (Square Wave Generator) is critical:
- Counter decrements by 2 each cycle
- Output toggles when counter reaches 0
- Counter reloads to initial value
- Produces 50% duty cycle square wave

### Frequency Calculation
- PIT base: 1.193182 MHz
- Formula: `frequency = 1193182 / count_register`
- Typical range: 100 Hz (count=11932) to 10000 Hz (count=119)

### Volume
Set square wave amplitude to 0.3 (-0.3 to +0.3) to avoid distortion and painful loudness.

### Synchronous vs Asynchronous
Rodio is asynchronous - it manages its own playback thread. Our implementation just updates the shared frequency value via `Arc<Mutex<f32>>`.

### Graceful Degradation
If Rodio fails to initialize (no audio device, permissions, etc.), log warning and fall back to `NullSpeaker`.

### WASM Considerations
For future WASM implementation:
- Web Audio API requires user interaction to start (browser policy)
- Use `OscillatorNode` with `type = "square"`
- Control frequency via `oscillator.frequency.setValueAtTime()`
- Connect/disconnect for enable/disable

## Testing Commands

```bash
# Build
cargo build

# Run basic beep test
./examples/run.sh beep

# Run with floppy (test DOS programs)
cargo run -p emu86-native -- --boot --floppy-a dos.img

# Check logs for I/O activity
tail -f emu86.log | grep -E "(0x004[0-3]|0x0061|Speaker)"
```

## Success Criteria

- [x] PIT ports 0x40-0x43 respond correctly
- [x] Port 0x61 bit 5 reflects Timer 2 output
- [x] Basic beep program produces audible 1000 Hz tone
- [x] Frequency changes are audible (sweep test)
- [x] DOS QBASIC BEEP command works
- [x] No audio device falls back gracefully to NullSpeaker
- [x] Logs show correct frequency calculations
