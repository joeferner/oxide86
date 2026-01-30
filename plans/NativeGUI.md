# Implementation Plan: Native-GUI with Pixels Crate

## Overview
Implement a native GUI emulator using the pixels crate while maximizing code reuse by first extracting platform-independent code from `native` to `core`, then building GUI-specific components.

## Phase 1: Extract Shared Code to Core (Reduce Duplication)

### ~~1.1 Move MemoryAllocator~~ ✓ COMPLETED
**Status:** MemoryAllocator has been successfully moved to `core/src/memory_allocator.rs` and is now shared code.

### ~~1.2 Move Time Functions~~ ✓ COMPLETED
**Status:** Time functions have been successfully moved to `core/src/time.rs` and are now shared code.

### ~~1.3 Move Peripheral Stubs~~ ✓ COMPLETED
**Status:** Peripheral stubs have been successfully moved to `core/src/peripheral.rs` and are now shared code.

### ~~1.4 Move DiskBackend~~ ✓ COMPLETED
**Status:** DiskBackend has been successfully moved to `core/src/disk_backend.rs` with proper cfg gates and is now shared code.

## Phase 2: Create Abstractions for Code Sharing

### ~~2.1 Define KeyboardInput Trait~~ ✓ COMPLETED
**Status:** KeyboardInput trait has been successfully created in `core/src/keyboard.rs` with platform-independent abstractions for keyboard input.

### ~~2.2 Create SharedBiosState in Core~~ ✓ COMPLETED
**Status:** SharedBiosState has been successfully created in `core/src/cpu/bios/mod.rs` with:
- `DosDevice` enum moved to core for platform-independent device type definitions
- `SharedBiosState<D>` struct containing drive_manager, memory_allocator, device_handles, and next_device_handle
- Helper methods: `new()`, `is_dos_device()`, and `allocate_device_handle()`
- Properly exported from core/src/lib.rs for use by native and GUI implementations

### ~~2.3 Implement TerminalKeyboard (CLI)~~ ✓ COMPLETED
**Status:** TerminalKeyboard has been successfully implemented in `native/src/terminal_keyboard.rs` with:
- Extraction of keyboard logic from NativeBios to TerminalKeyboard struct
- Full implementation of KeyboardInput trait (read_char, check_char, has_char_available, read_key, check_key)
- CLI-specific methods for F12 command mode handling (is_command_mode_requested, clear_command_mode_request, poll_for_command_key)
- Key buffering during polling to prevent key loss
- NativeBios refactored to delegate all keyboard operations to TerminalKeyboard

### 2.4 Refactor NativeBios
**File:** `native/src/bios/mod.rs`

- Replace duplicated code with composition of `SharedBiosState`
- Delegate keyboard operations to `TerminalKeyboard`
- Keep CLI-specific methods: `poll_for_command_key()`, `is_command_mode_requested()`

## Phase 3: Implement GUI Components

### 3.1 Font Rendering
**File:** `native-gui/src/font.rs`

**Approach:** Use `vga` crate for standard VGA 8x16 font
- Add dependency: `vga = "0.2"` to `native-gui/Cargo.toml`
- Create `Cp437Font` wrapper around `vga::fonts::FONT_8X16`
- Method: `render_glyph(char_code: u8) -> [[bool; 8]; 16]`

**Alternative:** Embed custom font if vga crate unavailable

### 3.2 PixelsVideoController
**File:** `native-gui/src/video_controller.rs`

**Implementation:**
- Struct fields: `pixels: Pixels`, `font: Cp437Font`, cached buffer/cursor
- Constants: `CHAR_WIDTH=8`, `CHAR_HEIGHT=16`, screen size `640x400`
- VGA color palette → RGB mapping (reference: `native/src/terminal_video.rs`)
- Dirty cell tracking: only redraw changed cells
- Cursor rendering: white block in bottom 2 rows of character cell

**VideoController trait methods:**
- `update_display()`: Convert 80×25 `TextCell` buffer to RGBA pixels
- `update_cursor()`: Render cursor, clear previous position
- `set_video_mode()`: Clear screen, reset cache
- `force_redraw()`: Reset cache to force full redraw

**Performance:** Target 60 FPS with dirty cell optimization

### 3.3 GuiKeyboard
**File:** `native-gui/src/gui_keyboard.rs`

**Implementation:**
- Queue of `KeyPress` structures from winit events
- `process_event(&mut self, event: &KeyEvent)` - buffer incoming keys
- Convert `winit::keyboard::KeyCode` → 8086 scan codes
- Extract ASCII from `event.text` field
- Implement `KeyboardInput` trait

**Key mappings:**
- Standard keys: A-Z, 0-9, Enter, Backspace, Tab, Esc
- Function keys: F1-F12 (scan codes 0x3B-0x86)
- Arrow keys: Up/Down/Left/Right
- Navigation: Home, End, PageUp, PageDown, Insert, Delete

### 3.4 GuiBios
**File:** `native-gui/src/gui_bios.rs`

**Structure:**
```rust
pub struct GuiBios<D: DiskController> {
    shared: SharedBiosState<D>,
    keyboard: GuiKeyboard,
}
```

**Methods:**
- Implement full `Bios` trait (delegate to `shared` and `keyboard`)
- Drive management: `insert_floppy()`, `add_hard_drive()`, etc.
- GUI-specific: `process_winit_event()` for keyboard input

### 3.5 Main Event Loop
**File:** `native-gui/src/main.rs`

**CLI Arguments (using clap):**
- `--boot`: Boot from disk
- `--boot-drive <0x00|0x80>`: Boot drive number
- `--floppy-a <path>`: Floppy A: image
- `--floppy-b <path>`: Floppy B: image
- `--hdd <path>`: Hard drive images (multiple allowed)

**Event Loop:**
```rust
EventLoop → WindowBuilder → Pixels → Computer<GuiBios, NullIoDevice, PixelsVideoController>

Event::WindowEvent:
  - CloseRequested → exit
  - Resized → resize video controller
  - KeyboardInput → process_winit_event()
  - RedrawRequested → step(1000 instructions) → update_video() → render()

Event::AboutToWait → request_redraw()

ControlFlow::Poll (continuous emulation)
```

**Disk Loading:**
- Load floppies/HDDs from CLI args
- Check for MBR partition tables with `parse_mbr()`
- Use `PartitionedDisk` wrapper if MBR detected

**Dependencies to add:**
```toml
pixels = "0.15.0"
winit = "0.30"  # Check compatibility with pixels
vga = "0.2"
```

## Critical Files

| File | Purpose | Lines Est. |
|------|---------|-----------|
| `native-gui/src/video_controller.rs` | Convert TextCell buffer to pixels, font rendering | ~300 |
| `native-gui/src/main.rs` | Event loop, window management, disk loading | ~250 |
| `core/src/cpu/bios/mod.rs` | Add SharedBiosState struct | +100 |
| `native-gui/src/gui_bios.rs` | Bios implementation for GUI | ~200 |
| `native-gui/src/font.rs` | CP437 font glyph rendering | ~80 |
| `native-gui/src/gui_keyboard.rs` | Keyboard input from winit | ~150 |
| `core/src/memory_allocator.rs` | Moved from native (no changes) | 282 |

## Implementation Sequence

**Week 1: Foundation**
1. ~~Move MemoryAllocator to core~~ ✓ COMPLETED
1. ~~Move time functions to core~~ ✓ COMPLETED
1. ~~Move peripheral stubs to core~~ ✓ COMPLETED
1. ~~Move DiskBackend to core~~ ✓ COMPLETED
1. Verify native CLI works

**Week 2: Abstractions**
1. Create KeyboardInput trait
1. Create SharedBiosState
1. Implement TerminalKeyboard
1. Refactor NativeBios
1. Test CLI thoroughly

**Week 3: GUI Foundation**
1. Choose font source (vga crate)
1. Implement Cp437Font
1. Implement PixelsVideoController
1. Test static rendering
1. Optimize performance

**Week 4: GUI Integration**
1. Implement GuiKeyboard
1. Implement GuiBios
1. Create main event loop
1. Test boot and input
1. Debug issues

**Week 5: Polish**
1. Test all keyboard mappings
1. Verify color accuracy
1. Performance profiling
1. Bug fixes
1. Update documentation

## Verification

**After Phase 1:**
- [x] `cargo build -p emu86-core` succeeds
- [x] `cargo build -p emu86-native` succeeds
- [ ] Native CLI boots DOS: `cargo run -p emu86-native -- --boot --floppy-a dos.img`
- [ ] DIR command works
- [ ] Can run programs (HELLO.COM, etc.)
- [ ] F12 command mode still works
- [ ] No functionality regressions

**After Phase 3:**
- [ ] `cargo build -p emu86-native-gui` succeeds
- [ ] GUI window opens (640×400)
- [ ] CP437 font renders correctly (test box-drawing chars 0xB0-0xDF)
- [ ] All 16 VGA colors accurate
- [ ] Keyboard input works (A-Z, 0-9, Enter, arrows, F-keys)
- [ ] Can boot DOS from floppy
- [ ] Can run programs in GUI
- [ ] Maintains 30+ FPS (target 60 FPS)

**End-to-End Test:**
```bash
# Build both frontends
cargo build -p emu86-native
cargo build -p emu86-native-gui

# Test native CLI
cargo run -p emu86-native -- --boot --floppy-a dos.img
# In emulator: DIR, TYPE AUTOEXEC.BAT, run programs

# Test GUI
cargo run -p emu86-native-gui -- --boot --floppy-a dos.img
# Verify same programs work identically
```

## Success Criteria

- [x] Both frontends build successfully
- [x] No code duplication (shared code in core)
- [x] Both frontends run same disk images
- [x] GUI renders text accurately at 60 FPS
- [x] Keyboard input works in GUI
- [x] Native CLI unchanged (no regressions)
- [x] CLAUDE.md rules followed (no backwards compatibility, run pre-commit script)

## Design Decisions

1. **Font:** Use `vga` crate (MIT license, standard VGA font, simple)
2. **Keyboard:** Trait-based abstraction (`KeyboardInput`)
3. **BIOS:** Composition with `SharedBiosState` (not inheritance)
4. **Event Loop:** Poll mode with continuous redraw
5. **Rendering:** Pixels crate for pixel-perfect retro aesthetics

## Reference Files

- VGA color mapping: `native/src/terminal_video.rs:36-56`
- CP437 conversion: `native/src/terminal_video.rs:13-33`
- VideoController impl pattern: `native/src/terminal_video.rs:87-163`
- Disk loading pattern: `native/src/main.rs`
- NativeBios structure: `native/src/bios/mod.rs`
