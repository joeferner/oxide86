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

**1.3 Update `core/src/memory.rs`**
- Add BDA mouse constants:
  - `BDA_MOUSE_X: usize = 0x80` (word)
  - `BDA_MOUSE_Y: usize = 0x82` (word)
  - `BDA_MOUSE_BUTTONS: usize = 0x84` (byte)
  - `BDA_MOUSE_VISIBLE: usize = 0x85` (byte, visibility counter)
  - `BDA_MOUSE_MIN_X/MAX_X: usize = 0x86/0x88` (words)
  - `BDA_MOUSE_MIN_Y/MAX_Y: usize = 0x8A/0x8C` (words)
- Update `initialize_bda()` to set mouse defaults

### Phase 2: Bios Mouse Integration

**2.1 Update `core/src/cpu/bios/mod.rs`**
- Add field to Bios struct: `pub mouse: Box<dyn MouseInput>`
- Update `new(keyboard: K, mouse: Box<dyn MouseInput>)` constructor
- Add mouse query methods:
  - `pub fn mouse_get_state(&self) -> MouseState`
  - `pub fn mouse_get_motion(&mut self) -> (i16, i16)`
  - `pub fn mouse_is_present(&self) -> bool`
- Bios remains `pub struct Bios<K: KeyboardInput>` (no M generic needed)

**2.2 Update `core/src/computer.rs`**
- No changes to Computer struct signature (stays `Computer<K: KeyboardInput, V: VideoController>`)
- Update `new()` to accept mouse parameter and pass to `Bios::new(keyboard, mouse)`
- Minimal changes to method signatures

### Phase 3: INT 33h Handler

**3.1 Create `core/src/cpu/bios/int33.rs`**

Implement DOS mouse interrupt handler with these essential functions:

| Function | Description |
|----------|-------------|
| AX=00h | Reset driver and read status (returns BX=0xFFFF if present) |
| AX=01h | Show cursor (increment visibility counter) |
| AX=02h | Hide cursor (decrement visibility counter) |
| AX=03h | Get position and button status |
| AX=04h | Set cursor position |
| AX=07h | Set horizontal min/max |
| AX=08h | Set vertical min/max |
| AX=0Bh | Read motion counters (mickeys) |

Key implementation details:
- Visibility counter: increment/decrement, visible when >= 0
- Coordinate clamping with min/max boundaries
- Motion counters reset on read
- Default resolution: 640x200 (8 mickeys per pixel)

**3.2 Update `core/src/cpu/bios/mod.rs`**
- Add `mod int33;`
- Register handler in `handle_bios_interrupt()`:
  ```rust
  0x33 => self.handle_int33(memory, io),
  ```

**3.3 Update `core/src/memory.rs`**
- Add INT 33h vector initialization in `initialize_ivt()`

### Phase 4: Native Terminal Implementation

**4.1 Create `native/src/terminal_mouse.rs`**

Implement `MouseInput` trait using crossterm's mouse events:

```rust
pub struct TerminalMouse {
    state: MouseState,
    motion_x: i16,
    motion_y: i16,
    last_col: u16,
    last_row: u16,
}
```

Methods:
- `pub fn new() -> Self`
- `pub fn poll_mouse_event(&mut self)` - Non-blocking poll for mouse events
- Implement `MouseInput` trait methods:
  - `get_state()` - Returns current MouseState
  - `get_motion()` - Returns and resets motion_x, motion_y
  - `is_present()` - Returns true
  - `process_cursor_moved(x, y)` - Receives terminal column/row, converts to DOS coords
  - `process_button(button, pressed)` - Updates button state

Coordinate conversion:
- Terminal coordinates: column (0-79), row (0-24) for 80x25 text mode
- DOS coordinates: multiply by 8 (640x200 resolution)
- Example: column 40, row 12 -> X=320, Y=96

**4.2 Update `native/src/main.rs`**
- Import `TerminalMouse`
- Enable mouse capture at startup:
  ```rust
  use crossterm::event::{EnableMouseCapture, DisableMouseCapture};
  execute!(stdout(), EnableMouseCapture)?;
  ```
- Disable on exit:
  ```rust
  execute!(stdout(), DisableMouseCapture)?;
  ```
- Construct and box the mouse implementation:
  ```rust
  let mouse: Box<dyn MouseInput> = Box::new(TerminalMouse::new());
  let bios: Bios<TerminalKeyboard> = Bios::new(keyboard, mouse);
  ```
- In main loop, poll for mouse events:
  ```rust
  if let Event::Mouse(mouse_event) = event::read()? {
      match mouse_event.kind {
          MouseEventKind::Down(button) | MouseEventKind::Up(button) => {
              let button_code = match button {
                  MouseButton::Left => 0,
                  MouseButton::Right => 1,
                  MouseButton::Middle => 2,
                  _ => continue,
              };
              let pressed = matches!(mouse_event.kind, MouseEventKind::Down(_));
              computer.bios_mut().mouse.process_button(button_code, pressed);
          }
          MouseEventKind::Moved | MouseEventKind::Drag(_) => {
              computer.bios_mut().mouse.process_cursor_moved(
                  mouse_event.column as f64,
                  mouse_event.row as f64
              );
          }
          _ => {}
      }
  }
  ```

**4.3 Add to `native/src/lib.rs`** (if exists, or add mod in main.rs)
- `pub mod terminal_mouse;`

**Crossterm Mouse Capabilities**:
- `MouseEventKind`: Down, Up, Drag, Moved, ScrollDown/Up/Left/Right
- `MouseEvent`: Contains kind, column, row, modifiers
- Enable with `EnableMouseCapture` command
- Platform limitations: Some terminals don't report all button states
- Character-based coordinates (80x25 typical)

### Phase 5: Native-GUI Implementation

**5.1 Create `native-gui/src/gui_mouse.rs`**

Implement `MouseInput` trait with:

```rust
pub struct GuiMouse {
    state: MouseState,          // Current position and buttons
    motion_x: i16,              // Accumulated mickeys X
    motion_y: i16,              // Accumulated mickeys Y
    last_x: f64,                // Last raw position for delta calculation
    last_y: f64,
}
```

Methods:
- `pub fn new() -> Self`
- Implement `MouseInput` trait methods:
  - `get_state()` - Returns current MouseState
  - `get_motion()` - Returns and resets motion_x, motion_y
  - `is_present()` - Returns true
  - `process_cursor_moved(x, y)` - Override to update position and accumulate deltas
  - `process_button(button, pressed)` - Override to update button state

Coordinate conversion:
- Convert winit window coordinates to DOS coordinates
- Handle scaling based on video mode (typically 640x200)
- Clamp to boundaries set via INT 33h AX=07h/08h

**5.2 Update `native-gui/src/main.rs`**
- Import `GuiMouse`
- Construct `GuiMouse::new()`
- In event loop, process mouse events:
  ```rust
  WindowEvent::CursorMoved { position, .. } => {
      computer.bios_mut().mouse.process_cursor_moved(position.x, position.y);
  }
  WindowEvent::MouseInput { button, state, .. } => {
      computer.bios_mut().mouse.process_button(button, state);
  }
  ```
- Pass to `Bios::new(keyboard, mouse)`
- Type signature remains simple:
  ```rust
  let mouse: Box<dyn MouseInput> = Box::new(GuiMouse::new());
  let bios: Bios<GuiKeyboard> = Bios::new(keyboard, mouse);
  ```

**Note**: Event processing methods are on the `MouseInput` trait, so event loop can call them directly:
  ```rust
  WindowEvent::CursorMoved { position, .. } => {
      computer.bios_mut().mouse.process_cursor_moved(position.x, position.y);
  }
  WindowEvent::MouseInput { button, state, .. } => {
      let button_code = match button {
          MouseButton::Left => 0,
          MouseButton::Right => 1,
          MouseButton::Middle => 2,
          _ => return,
      };
      let pressed = state == ElementState::Pressed;
      computer.bios_mut().mouse.process_button(button_code, pressed);
  }
  ```

**5.3 Add to `native-gui/src/lib.rs`** (if needed)
- Export `GuiMouse`

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
