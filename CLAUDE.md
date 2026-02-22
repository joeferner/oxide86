# Oxide86 - x86 Emulator

Intel x86 CPU emulator in Rust with native and WebAssembly support.

## Coding Rules

- Examples in x86 assembly; nasm assumed installed
- Avoid python; don't write tests unless directed
- Code in core must support both native and wasm
- No backwards compatibility
- Run ./scripts/pre-commit.sh when done; update CLAUDE.md for future edits
- Instead of running cargo build or clippy run ./scripts/pre-commit.sh instead
- logs are written to oxide86.log
- use Rust crates when possible
- when logging unimplemented features use log::warn!
- always write plans to the plans directory with a meaningful name
- when create test-programs place them in the appropriate directory and update test-programs/README.md
- when updating wasm use wasm/www/pkg/oxide86_wasm.d.ts interfaces instead of creating your own
- If the code you are creating is a major feature update README.md

### Logging Configuration

Control log levels via the `RUST_LOG` environment variable:

```bash
# Set everything to debug
RUST_LOG=debug cargo run -p oxide86-native-cli -- program.com

# Set specific module to debug
RUST_LOG=oxide86_core=debug cargo run -p oxide86-native-cli -- program.com

# Multiple modules with different levels
RUST_LOG=oxide86_core=debug,oxide86_native=trace cargo run -p oxide86-native-gui -- --boot --floppy-a dos.img

# Trace everything (very verbose)
RUST_LOG=trace cargo run -p oxide86-native-cli -- program.com
```

Default levels when `RUST_LOG` is not set:
- CLI/GUI: Error globally, Info for oxide86 modules
- GUI also filters wgpu_core=Info and wgpu_hal=Error to reduce graphics noise

## Architecture

### Workspace Structure
- **core/** - Platform-independent emulation (CPU, memory, instructions, drive management, BIOS)
- **native/** - CLI with TerminalKeyboard implementation
- **wasm/** - WebAssembly bindings for browser

### Key Files
| Path | Purpose |
|------|---------|
| `core/src/cpu/mod.rs` | CPU state, instruction dispatch, segment overrides |
| `core/src/cpu/bios/mod.rs` | Bios struct (generic over KeyboardInput & DiskController), interrupt dispatch |
| `core/src/cpu/bios/int*.rs` | Individual interrupt handlers |
| `core/src/memory.rs` | Memory model, BDA initialization |
| `core/src/drive_manager.rs` | Multi-drive management (DriveManager, DiskAdapter) |
| `core/src/disk.rs` | DiskImage, DiskGeometry, DiskController trait |
| `core/src/keyboard.rs` | KeyboardInput trait for platform-specific keyboard handling |
| `core/src/speaker.rs` | SpeakerOutput trait for platform-specific PC speaker emulation |
| `core/src/rodio_speaker.rs` | RodioSpeaker (native audio using Rodio library) |
| `core/src/io/mod.rs` | I/O port handling (PIT, keyboard controller, CGA, EGA ports) |
| `core/src/lib.rs` | Computer struct, boot process |
| `native/src/terminal_keyboard.rs` | TerminalKeyboard implementing KeyboardInput trait |
| `wasm/src/web_keyboard.rs` | WebKeyboard implementing KeyboardInput for browser |
| `wasm/src/web_speaker.rs` | WebSpeaker implementing SpeakerOutput using Web Audio API |
| `wasm/src/web_video.rs` | WebVideo rendering to HTML5 Canvas |
| `wasm/src/web_mouse.rs` | WebMouse implementing MouseInput for browser |

## Implementation Notes

### Flags
Use `set_flag(flag, bool)` only - no `clear_flag` method exists.
```rust
self.set_flag(cpu_flag::CARRY, true);   // set
self.set_flag(cpu_flag::CARRY, false);  // clear
```

### Segment Overrides
- CPU field: `segment_override: Option<u16>`
- Checked by: `decode_modrm()`, `mov_acc_moffs()`, `xlat()`, string source ops (DS:SI)
- String destinations (ES:DI) cannot be overridden per x86 spec

### Repeat Prefixes
- CPU field: `repeat_prefix: Option<RepeatPrefix>`
- 0xF3 (REP/REPE): MOVS, STOS, LODS, CMPS, SCAS
- 0xF2 (REPNE): CMPS, SCAS

### Video Mode Column/Row Handling

**Critical:** Never hardcode 80x25 dimensions when working with video modes.

- Internal buffer is always 80x25, but modes 0x00/0x01 use 40x25 addressing
- Always use `video.get_cols()` and `video.get_rows()` instead of hardcoded values
- Memory addressing translates 40-column offsets to 80-column buffer indices
- Rendering loops must use actual mode dimensions, not `TEXT_MODE_COLS`/`TEXT_MODE_ROWS`
- Files with video mode logic: `core/src/video.rs`, `core/src/cpu/bios/int10.rs`, `native-gui/src/gui_video.rs`, `wasm/src/web_video.rs`, `native-cli/src/terminal_video.rs`

### Multi-Drive Management (core/src/drive_manager.rs)

The `DriveManager` struct manages multiple floppy and hard drives:

**Drive Numbering:**
- `0x00` = Floppy A:, `0x01` = Floppy B:
- `0x80` = Hard drive C:, `0x81` = D:, etc.

**Key Structures:**
```rust
DriveState<D>      // Per-drive state: adapter, current_dir, disk_changed, removable
DriveManager<D>    // Holds floppy_drives[2], hard_drives: Vec, open_files, searches
DiskAdapter<D>     // Wraps DiskController for fatfs Read/Write/Seek traits
```

**IMPORTANT - BDA Hard Drive Count:**
- The BIOS Data Area (BDA) stores hard drive count at offset 0x75
- This is initialized in `Computer::new()` to the count at that moment
- **CRITICAL**: After adding/removing hard drives at runtime, you MUST call `computer.update_bda_hard_drive_count()`
- Failure to update the BDA causes boot failures (BIOS functions check this value)
- This is automatically done in WASM `set_hard_drive()` and native startup

**Floppy Hot-Swap:**
- `insert_floppy(slot, disk)` - Sets `disk_changed = true`
- `eject_floppy(slot)` - Closes open files, returns disk
- `disk_detect_change(drive)` - Returns and clears change flag (INT 13h AH=16h)

**Per-Drive State:**
- Each drive has its own `current_dir: String`
- Path parsing extracts drive letter: `C:\FOO` -> (0x80, "/FOO")
- File handles store which drive they belong to

**Handle Allocation:**
- Single global `next_handle` counter in DriveManager (starts at 3)
- NativeBios device handles share same space, synced via `set_next_handle()`

**DiskAdapter Usage:**
- Call `reset_position()` before each `fatfs::FileSystem::new()`
- Extract path/position data before mutable adapter access (borrow checker)

**Partition Support:**
- Hard drives are checked for MBR partition tables on load
- If MBR detected, `PartitionedDisk` wrapper offsets all sector access to partition 1 start
- `parse_mbr()` extracts up to 4 partition entries from sector 0
- Boot sector (MBR) remains accessible via raw disk for booting
- DOS file operations see only the partition, not the full disk

### Bios Structure and Handle Management
- `Bios<K: KeyboardInput, D: DiskController>` - concrete struct in core (not a trait)
- Generic over keyboard input handler and disk controller for platform flexibility
- Contains `SharedBiosState<D>` with drive manager, memory allocator, device handles
- Device handles (CON, NUL, etc.) and file handles share same number space
- `device_handles: HashMap<u16, DosDevice>` for devices in SharedBiosState
- File handles managed by DriveManager
- Sync via `set_next_handle()` to prevent collisions
- Platform-specific implementations provide KeyboardInput (e.g., TerminalKeyboard for CLI)

### Timer Emulation
- BDA offset 0x6C: 32-bit tick counter (18.2 Hz)
- Auto-increments via `Computer::increment_cycles()` after each instruction
- Initialized from host time via `Bios::get_system_ticks()`

### PC Speaker / Sound Emulation

**Hardware Overview:**
- PC speaker controlled via Intel 8253/8254 Programmable Interval Timer (PIT)
- Base frequency: 1.193182 MHz
- Output frequency: `PIT_FREQUENCY_HZ / count_register` Hz
- Enabled via Port 0x61 bits: 0x01 (PIT Ch2 gate) + 0x02 (speaker data)

**Implementation (`core/src/io/pit.rs`):**
- PIT Channel 2 generates square wave (Mode 3) for speaker
- `update_speaker()` called every ~100 CPU cycles in `core/src/computer.rs`
- Reads PIT count register and port 0x61 control bits
- Calls `speaker.set_frequency(enabled, frequency)` on SpeakerOutput trait

**Platform-Specific Implementations:**

*Native (RodioSpeaker):*
- Uses Rodio audio library for native output
- 48kHz sample rate, square wave generation
- 30% volume to avoid distortion
- Enabled with `audio-rodio` feature flag
- Graceful fallback to NullSpeaker if audio device unavailable

*WASM (WebSpeaker):*
- Uses Web Audio API (OscillatorNode with "square" type)
- 48kHz sample rate (browser default)
- 30% volume (GainNode)
- Implements `unsafe Send` trait (safe in single-threaded WASM)
- Graceful fallback to NullSpeaker if Web Audio API fails
- Note: Modern browsers require user interaction before audio plays (autoplay policy)

**Adding New Platform:**
1. Implement `SpeakerOutput` trait with `set_frequency()` and `update()` methods
2. For WASM targets, add `unsafe impl Send` (safe due to single-threaded environment)
3. Return boxed implementation from platform initialization
4. Provide fallback to `NullSpeaker` on initialization failure

### AdLib Sound Card (OPL2 FM Synthesis)

**Hardware Overview:**
- Yamaha YM3812 (OPL2) FM synthesizer chip on AdLib Music Synthesizer Card (1987)
- 9 FM channels, 2 operators each (18 operators total)
- Ports: 0x388 (address/status), 0x389 (data)
- Standard AdLib detection: write timer values, read status, check flags

**Implementation (`core/src/audio/`):**
- `opl2.rs` — hand-rolled OPL2 emulator: ADSR envelopes, 4 waveforms, tremolo/vibrato LFOs, timers
  - Internal rate: 49716 Hz, downsampled to 44100 Hz output
  - `generate_samples(cpu_cycles, out)` — produces f32 PCM samples
- `adlib.rs` — `Adlib` struct: owns `Opl2` + `Arc<Mutex<VecDeque<f32>>>` ring buffer. Implements `SoundCard`.
  - `Adlib::consumer()` — returns `AdlibConsumer` handle (cloneable, for native audio thread)
  - `AdlibConsumer::pop_samples()` — drains samples from the shared ring buffer
- `mod.rs` — `SoundCard` trait (`write_port`, `read_port`, `port_ranges`, `tick`, `pop_samples`, `reset`), `NullSoundCard`, `SoundCardType` enum

**I/O Routing (`core/src/io/mod.rs`):**
- `IoDevice` holds `sound_card: Box<dyn SoundCard>` (default: `NullSoundCard`)
- Port 0x388/0x389 reads/writes routed through `sound_card.read_port()` / `sound_card.write_port()`
- `IoDevice::set_sound_card(card)` — installs a new sound card
- `IoDevice::tick_sound_card(cycles)` — advances chip and accumulates samples
- `IoDevice::pop_sound_card_samples(count)` — drains samples for audio output

**Computer Integration (`core/src/computer.rs`):**
- `set_sound_card(card: Box<dyn SoundCard>)` — delegates to `io_device.set_sound_card()`
- `get_adlib_samples(count)` — delegates to `io_device.pop_sound_card_samples()`
- Every instruction: `io_device.tick_sound_card(cycles)` (always; NullSoundCard is a no-op)

**CLI Usage:**
```bash
cargo run -p oxide86-native-gui -- --sound-card adlib test-programs/audio/adlib_detection.com
cargo run -p oxide86-native-gui -- --sound-card adlib --boot --floppy-a dos.img
```

**Native Platform (`native-common/src/`):**
- `setup.rs::create_adlib(sound_card)` — creates `Adlib`, gets `AdlibConsumer` handle, boxes into `Box<dyn SoundCard>`, returns `(card, RodioAdlib)`
- Caller: `computer.set_sound_card(card)`, keep `_adlib_sink` alive in `main()` scope
- `rodio_adlib.rs::RodioAdlib` — Rodio `Sink` with `AdlibSource` draining via `AdlibConsumer`

**WASM Platform:**
- Sound card created at `Oxide86Computer::new()` when `sound_card: "adlib"` is in config
- `enable_adlib() -> u32` — satisfies browser autoplay policy; returns sample rate (44100)
- `get_adlib_samples(count) -> Float32Array` — pops samples from the Adlib's internal buffer
- `wasm/www/src/emulatorState.ts`: `setupAdlibAudio()` creates AudioWorklet at 44100 Hz; posts 2048 samples/frame via MessagePort

**Test Program:** `test-programs/audio/adlib_detection.asm` — IBM AdLib detection + two-note playback

### Keyboard Controller / A20 Line

**Hardware Overview:**
- Keyboard controller is an Intel 8042 microcontroller on AT-class PCs
- Controls keyboard communication and system functions including A20 line
- Port 0x60: Data port (keyboard scan codes, command data)
- Port 0x64: Command/status port

**A20 Line:**
- Address line 20 (bit 20) controls access to memory above 1 MB
- Disabled (A20=0): Addresses wrap at 1 MB (8086/8088 compatibility mode)
- Enabled (A20=1): Full address space accessible (AT-class and later)
- Controlled via keyboard controller output port bit 1
- Most boot loaders and operating systems enable A20 during initialization

**Implementation (`core/src/io/mod.rs`):**
- Status register (port 0x64): Always returns 0x14 (ready for commands, system flag set)
- Output port: 8-bit register, bit 1 controls A20 gate (enabled by default)
- Supported commands via port 0x64:
  - `0xD0`: Read output port (next read from 0x60 returns output port value)
  - `0xD1`: Write output port (next write to 0x60 updates output port)
  - `0xDD`: Enable A20 line (set bit 1 of output port)
  - `0xDF`: Disable A20 line (clear bit 1 of output port)

**Memory Integration (`core/src/memory.rs`):**
- Memory subsystem tracks A20 state via `a20_enabled` flag
- `apply_a20_gate()` implements address translation:
  - A20 enabled: Full 20-bit addressing (addresses >= 1MB return 0xFF/ignored)
  - A20 disabled: Bit 20 masked off (wraps at 1MB like 8086/8088)
- `Computer::step()` syncs A20 state from IoDevice to Memory after each instruction
- This allows boot loaders to test A20 functionality by writing to 0x000000 and 0x100000

## BIOS/DOS Interrupts

### Implemented Interrupts
| INT | Service | Key Functions |
|-----|---------|---------------|
| 09h | Keyboard IRQ | Hardware interrupt (stub - buffer populated by fire_keyboard_irq) |
| 10h | Video | 00h set mode, 02h cursor, 0Eh teletype, 11h char gen, 15h display params, 1Ah display code, FEh get buffer |
| 12h | Memory | Returns AX = KB (typically 640) |
| 13h | Disk | 00h reset, 02h read, 03h write, 04h verify, 05h format, 08h params, 15h type, 16h change, 18h DASD |
| 14h | Serial | 00h init, 01h write, 02h read, 03h status |
| 15h | System | 86h wait, 88h ext mem, C0h config |
| 16h | Keyboard | 00h read, 01h check, 02h shift flags, 10h ext read, 11h ext check, 12h ext flags |
| 17h | Printer | 00h write, 01h init, 02h status |
| 1Ah | Time | 00h get ticks, 01h set, 02h RTC |
| 20h | Terminate | Program terminate (halt CPU) |
| 21h | DOS | Console, files, dirs, memory, exec |
| 28h | Idle | TSR hook during keyboard wait |
| 29h | FastCon | AL = char to output |
| 2Ah | Network | Installation check, critical sections |
| 2Fh | Multiplex | TSR checks (return AL=0 not installed) |

### INT 21h DOS Functions
- **Console**: 01h read+echo, 02h write, 09h string
- **Files**: 3Ch create, 3Dh open, 3Eh close, 3Fh read, 40h write, 41h delete, 42h seek, 44h IOCTL, 45h dup
- **Dirs**: 39h mkdir, 3Ah rmdir, 3Bh chdir, 47h getcwd, 4Eh/4Fh find
- **Memory**: 48h alloc, 49h free, 4Ah resize
- **Process**: 4Bh exec, 4Ch exit, 50h set PSP
- **System**: 0Eh select disk, 19h get drive, 25h/35h int vectors, 2Ah get date, 2Bh set date, 2Ch get time, 2Dh set time, 30h version, 36h disk free space

### Adding New Interrupt
1. Create `core/src/cpu/bios/intXX.rs` with `handle_intXX()` method
2. Add `mod intXX;` in `core/src/cpu/bios/mod.rs`
3. Add case to `handle_bios_interrupt()` dispatch

### DOS Error Codes
`SUCCESS=0, FILE_NOT_FOUND=2, PATH_NOT_FOUND=3, TOO_MANY_FILES=4, ACCESS_DENIED=5, INVALID_HANDLE=6, NO_MORE_FILES=0x12`

## Running Programs

### native-cli and native-gui

Both native-cli and native-gui support two modes: loading programs or booting from disk.

**Load a program (.COM file):**
```bash
# CLI version
cargo run -p oxide86-native-cli -- program.com

# GUI version
cargo run -p oxide86-native-gui -- program.com

# With custom segment:offset
cargo run -p oxide86-native-cli -- program.com --segment 0x1000 --offset 0x0000
```

**Boot from disk:**
```bash
# Boot from floppy A:
cargo run -p oxide86-native-cli -- --boot --floppy-a dos.img
cargo run -p oxide86-native-gui -- --boot --floppy-a dos.img

# Boot from hard drive C: with floppy in B:
cargo run -p oxide86-native-cli -- --boot --boot-drive 0x80 --hdd drive_c.img --floppy-b disk2.img

# Multiple hard drives
cargo run -p oxide86-native-cli -- --boot --hdd drive_c.img --hdd drive_d.img
```

**CLI Options:**
- `<program>` - Path to program binary (required unless --boot)
- `--boot` - Boot from disk instead of loading program
- `--cpu <type>` - CPU type to emulate: 8086, 286, 386, 486 (default: 8086)
- `--segment <hex>` - Starting segment (default: 0x0000)
- `--offset <hex>` - Starting offset (default: 0x0100 for .COM files)
- `--floppy-a <path>` - Floppy A: image
- `--floppy-b <path>` - Floppy B: image
- `--hdd <path>` - Hard drive image (can specify multiple for C:, D:, etc.)
- `--boot-drive <0x00|0x01|0x80>` - Boot drive number (default: 0x00)

### WASM

**Load a program:**
```javascript
const computer = new Oxide86Computer("canvas-id");
const programData = new Uint8Array([...]); // Your .COM file bytes
computer.load_program(programData, 0x0000, 0x0100); // segment:offset
```

**Boot from disk:**
```javascript
const computer = new Oxide86Computer("canvas-id");
const diskImage = new Uint8Array([...]); // Your disk image bytes
computer.load_floppy(0, diskImage); // Load into drive A:
computer.boot(0x00); // Boot from drive A:
```

**Boot Sequence:**
1. Read sector 0 to 0x7C00
2. Verify 0x55AA signature at bytes 510-511
3. Set CS:IP=0:7C00, DL=drive, SS:SP=0:7C00

**CPU Types:**
The `--cpu` option controls which CPU type is emulated:
- `8086` (default): Original 8086, 1 MB addressable, no extended memory
- `286`: 80286 with 16 MB addressable, 15 MB extended memory
- `386`: 80386 with 32-bit support, 64 MB extended memory (max for INT 15h AH=88h)
- `486`: 80486 with 32-bit support, 64 MB extended memory (max for INT 15h AH=88h)

Currently, the CPU type primarily affects INT 15h AH=88h (Get Extended Memory Size).
Future work may add support for protected mode and 32-bit instructions on 286+/386+.

## Development

```bash
cargo build                          # all crates
cargo run -p oxide86-native-cli -- <args>  # run native
cargo clippy                         # lint
```

## Resources
- [8086 User Manual](https://edge.edx.org/c4x/BITSPilani/EEE231/asset/8086_family_Users_Manual_1_.pdf)
- [x86 Reference](https://www.felixcloutier.com/x86/)
- [8086 Opcodes](http://www.mlsite.net/8086/)
