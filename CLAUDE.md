# emu86 - 8086 Emulator

Intel 8086 CPU emulator in Rust with native and WebAssembly support.

## Coding Rules

- Examples in x86 assembly; nasm assumed installed
- Avoid python; don't write tests unless directed
- Code in core must support both native and wasm
- No backwards compatibility
- Run ./scripts/pre-commit.sh when done; update CLAUDE.md for future edits
- logs are written to emu86.log
- use Rust crates when possible
- when logging unimplemented features use log::warn!
- always write plans to the plans directory with a meaningful name
- when create test-programs place them in the appropriate directory and update test-programs/README.md
- when updating wasm use wasm/www/pkg/emu86_wasm.d.ts interfaces instead of creating your own

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
- This is automatically done in WASM `add_hard_drive()` and native startup

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
- Output frequency: `1193182 / count_register` Hz
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
- **System**: 0Eh select disk, 19h get drive, 25h/35h int vectors, 30h version, 36h disk free space

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
cargo run -p emu86-native-cli -- program.com

# GUI version
cargo run -p emu86-native-gui -- program.com

# With custom segment:offset
cargo run -p emu86-native-cli -- program.com --segment 0x1000 --offset 0x0000
```

**Boot from disk:**
```bash
# Boot from floppy A:
cargo run -p emu86-native-cli -- --boot --floppy-a dos.img
cargo run -p emu86-native-gui -- --boot --floppy-a dos.img

# Boot from hard drive C: with floppy in B:
cargo run -p emu86-native-cli -- --boot --boot-drive 0x80 --hdd drive_c.img --floppy-b disk2.img

# Multiple hard drives
cargo run -p emu86-native-cli -- --boot --hdd drive_c.img --hdd drive_d.img
```

**CLI Options:**
- `<program>` - Path to program binary (required unless --boot)
- `--boot` - Boot from disk instead of loading program
- `--segment <hex>` - Starting segment (default: 0x0000)
- `--offset <hex>` - Starting offset (default: 0x0100 for .COM files)
- `--floppy-a <path>` - Floppy A: image
- `--floppy-b <path>` - Floppy B: image
- `--hdd <path>` - Hard drive image (can specify multiple for C:, D:, etc.)
- `--boot-drive <0x00|0x01|0x80>` - Boot drive number (default: 0x00)

### WASM

**Load a program:**
```javascript
const computer = new Emu86Computer("canvas-id");
const programData = new Uint8Array([...]); // Your .COM file bytes
computer.load_program(programData, 0x0000, 0x0100); // segment:offset
```

**Boot from disk:**
```javascript
const computer = new Emu86Computer("canvas-id");
const diskImage = new Uint8Array([...]); // Your disk image bytes
computer.load_floppy(0, diskImage); // Load into drive A:
computer.boot(0x00); // Boot from drive A:
```

**Boot Sequence:**
1. Read sector 0 to 0x7C00
2. Verify 0x55AA signature at bytes 510-511
3. Set CS:IP=0:7C00, DL=drive, SS:SP=0:7C00

## Development

```bash
cargo build                          # all crates
cargo run -p emu86-native-cli -- <args>  # run native
cargo clippy                         # lint
```

## Resources
- [8086 User Manual](https://edge.edx.org/c4x/BITSPilani/EEE231/asset/8086_family_Users_Manual_1_.pdf)
- [x86 Reference](https://www.felixcloutier.com/x86/)
- [8086 Opcodes](http://www.mlsite.net/8086/)
