# Implementation Plan: Mouse Support for emu86

## Overview

Add DOS-compatible mouse support (INT 33h) to the emu86 emulator, supporting both native (terminal) and native-gui environments. The implementation follows the established keyboard architecture pattern using generic traits.

## Architecture Decisions

### 1. MouseInput Trait Pattern
Following the existing `KeyboardInput` trait architecture:
- Platform-independent `MouseInput` trait in core
- Platform-specific implementations (`GuiMouse`, `NullMouse`)
- Generic composition in `Bios<K, M>` and `Computer<K, M, V>`

### 2. Trait Design
```rust
pub trait MouseInput {
    // Core methods for INT 33h
    fn get_state(&self) -> MouseState;        // Current position and buttons
    fn get_motion(&mut self) -> (i16, i16);   // Delta movement (mickeys)
    fn is_present(&self) -> bool;              // Hardware detection

    // Event processing (default no-ops for platforms without events)
    fn process_cursor_moved(&mut self, _x: f64, _y: f64) {}
    fn process_button(&mut self, _button: u8, _pressed: bool) {}
}

pub struct MouseState {
    pub x: u16,
    pub y: u16,
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
}
```

**Important**: The trait must be object-safe for `Box<dyn MouseInput>` to work:
- All methods take `&self` or `&mut self` (no `Self` in parameters)
- No generic methods
- No associated types (or use where clauses if needed)

**Design note**: Event processing methods have default no-op implementations. NullMouse uses defaults, GuiMouse overrides them. This allows event loop to call methods directly on the trait object.

### 3. Bios Mouse Integration
Add mouse as a field using dynamic dispatch: `Box<dyn MouseInput>`:
- Avoids generic type proliferation (no M parameter needed)
- Simpler type signatures throughout codebase
- Minimal runtime overhead (trait object dispatch)
- Keyboard stays generic, mouse uses dynamic dispatch

### 4. Platform Implementations
- **Native**: Use `NullMouse` (no mouse support in terminal mode for simplicity)
- **Native-GUI**: Full `GuiMouse` implementation using winit events

### 5. BDA Storage
New mouse state fields in BIOS Data Area (0x40:0x80-0x8C):
- Position (X, Y)
- Button state
- Visibility counter
- Min/Max boundaries

## Implementation Steps

### Phase 1: Core Infrastructure

**1.1 Create `core/src/mouse.rs`** ✓ COMPLETED
- Define `MouseInput` trait with methods:
  - Core methods: `get_state()`, `get_motion()`, `is_present()`
  - Event processing methods with default no-op implementations: `process_cursor_moved()`, `process_button()`
- Define `MouseState` struct
- Implement `NullMouse` (uses default trait implementations, returns "not present")
- Add comprehensive documentation
- Ensure trait is object-safe for `Box<dyn MouseInput>`

**1.2 Update `core/src/lib.rs`** ✓ COMPLETED
- Add `pub mod mouse;`
- Export `MouseInput`, `MouseState`, `NullMouse`

**1.3 Update `core/src/memory.rs`** ✓ COMPLETED
- Add BDA mouse constants:
  - `BDA_MOUSE_X: usize = 0x80` (word)
  - `BDA_MOUSE_Y: usize = 0x82` (word)
  - `BDA_MOUSE_BUTTONS: usize = 0x84` (byte)
  - `BDA_MOUSE_VISIBLE: usize = 0x85` (byte, visibility counter)
  - `BDA_MOUSE_MIN_X/MAX_X: usize = 0x86/0x88` (words)
  - `BDA_MOUSE_MIN_Y/MAX_Y: usize = 0x8A/0x8C` (words)
- Update `initialize_bda()` to set mouse defaults

### Phase 2: Bios Mouse Integration

**2.1 Update `core/src/cpu/bios/mod.rs`** ✓ COMPLETED
- Add field to Bios struct: `pub mouse: Box<dyn MouseInput>`
- Update `new(keyboard: K, mouse: Box<dyn MouseInput>)` constructor
- Add mouse query methods:
  - `pub fn mouse_get_state(&self) -> MouseState`
  - `pub fn mouse_get_motion(&mut self) -> (i16, i16)`
  - `pub fn mouse_is_present(&self) -> bool`
- Bios remains `pub struct Bios<K: KeyboardInput>` (no M generic needed)
- Updated native/src/main.rs and native-gui/src/main.rs to pass NullMouse to Bios::new()

**2.2 Update `core/src/computer.rs`** ✓ COMPLETED
- No changes to Computer struct signature (stays `Computer<K: KeyboardInput, V: VideoController>`)
- Update `new()` to accept mouse parameter and pass to `Bios::new(keyboard, mouse)`
- Minimal changes to method signatures
- Updated native/src/main.rs and native-gui/src/main.rs to use new constructor pattern

### Phase 3: INT 33h Handler

**3.1 Create `core/src/cpu/bios/int33.rs`** ✓ COMPLETED

**3.2 Update `core/src/cpu/bios/mod.rs`** ✓ COMPLETED

**3.3 Update `core/src/memory.rs`** ✓ COMPLETED (already handled by generic IVT initialization)

### Phase 4: Native Terminal Implementation ✓ COMPLETED

**4.1 Create `native/src/terminal_mouse.rs`** ✓ COMPLETED
- Created TerminalMouse struct with MouseState, motion tracking, and position fields
- Implemented MouseInput trait methods:
  - `get_state()` - Returns current MouseState
  - `get_motion()` - Returns and resets motion_x, motion_y
  - `is_present()` - Returns true
  - `process_cursor_moved(x, y)` - Converts terminal column/row to DOS coords
  - `process_button(button, pressed)` - Updates button state
- Coordinate conversion implemented (terminal chars × 8 = DOS pixels)
- Added module declaration to `native/src/main.rs`
- Updated Computer construction to use `Box::new(TerminalMouse::new())`

**4.2 Refactor Event Polling Architecture** ✓ COMPLETED
- Added `process_event(event: Event)` method to TerminalKeyboard
- Removed `poll_for_command_key()` method (replaced by centralized polling)
- Centralized event polling now implemented in main.rs run() function
- Single event poll dispatches to both keyboard and mouse handlers
- Keyboard events dispatched to `keyboard.process_event()`
- Mouse move/drag events dispatched to `mouse.process_cursor_moved()`
- Mouse button events dispatched to `mouse.process_button()`

**4.3 Enable Mouse Capture** ✓ COMPLETED
- Added imports for `EnableMouseCapture` and `DisableMouseCapture`
- Enabled mouse capture at startup (before run() call)
- Disabled mouse capture after run() completes
- Added mouse capture disable to panic handler for clean recovery
- Terminal mouse now fully functional with crossterm event stream

### Phase 5: Native-GUI Implementation ✓ COMPLETED

**5.1 Create `native-gui/src/gui_mouse.rs`** ✓ COMPLETED
- Created GuiMouse struct with MouseState, motion tracking, and window size tracking
- Implemented MouseInput trait methods:
  - `get_state()` - Returns current MouseState
  - `get_motion()` - Returns and resets motion_x, motion_y
  - `is_present()` - Returns true
  - `process_cursor_moved(x, y)` - Converts window coords to DOS coords, accumulates motion
  - `process_button(button, pressed)` - Updates button state
- Coordinate conversion from window space (640x400) to DOS space (640x200)
- Motion tracking in mickeys with 8 mickeys per pixel ratio

**5.2 Update `native-gui/src/main.rs`** ✓ COMPLETED
- Imported GuiMouse
- Added mouse event imports from winit (MouseButton, ElementState)
- Constructed GuiMouse with window dimensions
- Added event handlers for WindowEvent::CursorMoved and WindowEvent::MouseInput
- Mouse button mapping (Left=0, Right=1, Middle=2)

**5.3 Add to `native-gui/src/lib.rs`** ✓ COMPLETED (not needed - no lib.rs exists)

### Phase 6: Testing and Refinement

**Test Coverage**:
1. Mouse detection (INT 33h AX=00h returns BX=0xFFFF)
2. Position tracking and reading (AX=03h)
3. Button press/release detection
4. Cursor visibility toggling (AX=01h/02h)
5. Boundary clamping (AX=07h/08h)
6. Motion counters (AX=0Bh)

**Edge Cases**:
- Visibility counter behavior (can go negative, visible only when >= 0)
- Coordinate clamping at min/max limits
- Motion counter accumulation and reset
- Multiple button presses simultaneously

## Files Summary

### Files to CREATE:
1. `/home/fernejo/dev/emu86/core/src/mouse.rs` - MouseInput trait, MouseState, NullMouse
2. `/home/fernejo/dev/emu86/core/src/cpu/bios/int33.rs` - INT 33h handler
3. `/home/fernejo/dev/emu86/native/src/terminal_mouse.rs` - Terminal mouse implementation
4. `/home/fernejo/dev/emu86/native-gui/src/gui_mouse.rs` - GUI mouse implementation

### Files to MODIFY:
1. `/home/fernejo/dev/emu86/core/src/lib.rs` - Export mouse module
2. `/home/fernejo/dev/emu86/core/src/memory.rs` - Add BDA mouse constants and initialization
3. `/home/fernejo/dev/emu86/core/src/cpu/bios/mod.rs` - Add mouse field, mouse methods, register INT 33h
4. `/home/fernejo/dev/emu86/core/src/computer.rs` - Update constructor to pass mouse to Bios
5. `/home/fernejo/dev/emu86/native/src/main.rs` - Enable mouse capture, construct TerminalMouse, poll events
6. `/home/fernejo/dev/emu86/native-gui/src/main.rs` - Construct GuiMouse, process events

## Critical Implementation Notes

### Box<dyn MouseInput> Approach
Using dynamic dispatch instead of generics for mouse support:
- **Pros**: No generic type proliferation, simpler signatures, more flexible
- **Cons**: Minor runtime overhead from virtual dispatch, heap allocation
- **Pattern**: `Box::new(NullMouse::new())` or `Box::new(GuiMouse::new())`
- Bios remains `Bios<K: KeyboardInput>` (only keyboard is generic)
- Changes localized to Bios constructor and platform main.rs files

### Event Processing Through Trait
Event processing methods are part of the `MouseInput` trait:
- Default implementations are no-ops (used by NullMouse)
- GuiMouse overrides to update internal state
- Event loop calls methods directly on `Box<dyn MouseInput>`
- No downcasting or separate storage needed

### DOS Coordinate System
- Default: 640x200 (0-639 horizontal, 0-199 vertical)
- Text mode: Divide by 8 for character coordinates
- Graphics mode: Direct pixel mapping
- Scaling handled by INT 33h implementation based on current video mode

### Mickeys (Motion Units)
- Raw mouse movement units
- Default ratio: 8 mickeys per pixel
- Accumulate between reads
- Reset on INT 33h AX=0Bh
- Independent of position tracking

## Verification Steps

1. **Compile all crates**: Ensure no compilation errors after generic changes
   ```bash
   cargo build
   cargo clippy
   ```

2. **Run native**: Test terminal mouse support
   ```bash
   cargo run -p emu86-native -- --boot --floppy-a dos.img
   # Move mouse in terminal (if supported)
   # Click buttons and verify DOS detects them
   ```

3. **Run native-gui**: Test GUI mouse detection and movement
   ```bash
   cargo run -p emu86-native-gui -- --boot --floppy-a dos.img
   ```

4. **Test DOS program**: Create simple NASM test program:
   ```nasm
   ; Test INT 33h mouse detection
   mov ax, 0x00      ; Reset and detect
   int 0x33
   ; BX should be 0xFFFF if mouse present
   ```

5. **Interactive testing in GUI**:
   - Move mouse cursor in window
   - Click buttons
   - Verify position updates via INT 33h AX=03h
   - Test boundary limits via AX=07h/08h

## Expected Outcomes

- DOS programs can detect mouse via INT 33h in both terminal and GUI modes
- Mouse position tracking works in both environments:
  - Terminal: Character-based coordinates (80x25) converted to DOS coords
  - GUI: Pixel-accurate positioning
- Button clicks registered correctly (left, right, middle buttons)
- Clean, platform-independent architecture
- Simpler type signatures (no M generic parameter)
- Both terminal and GUI have full mouse support
- Minimal runtime overhead from trait object dispatch

## Future Enhancements

- Add terminal mouse support using crossterm events (replace NullMouse in native)
- Implement advanced INT 33h functions (AX=05h, 06h, 09h, 0Ah, 0Ch, 0Fh)
- Add mouse cursor rendering in GUI (custom sprite overlay)
- Test with real DOS programs (Norton Commander, DOS Shell, games)
