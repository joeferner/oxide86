# Joystick (Game Port) Implementation Plan

## Context

The PC game port (port 0x201) is a standard IBM PC peripheral interface for analog joysticks.
It is listed as a planned feature in the README but has no implementation — only a commented-out
TODO in `native-gui/src/main.rs:269`. This plan adds:

1. A `JoystickDevice` in `core` implementing the gameport protocol at I/O port 0x201
2. A `GilrsJoystick` wrapper in `native-common` that reads real gamepad input via the `gilrs` crate
3. `GilrsJoystick` is created inside `create_computer` when `--joystick` is specified, and returned to callers
4. Wiring in `native-gui` and `native-cli` to poll the returned `GilrsJoystick`

---

## PC Gameport Protocol (0x201)

**Write to 0x201** — any value starts the one-shot timing circuit (resets all axis timers).

**Read from 0x201** — returns an 8-bit status byte:

| Bit | Description |
|-----|-------------|
| 7   | Button 4 (0 = pressed) |
| 6   | Button 3 (0 = pressed) |
| 5   | Button 2 (0 = pressed) |
| 4   | Button 1 (0 = pressed) |
| 3   | Joystick 2 Y-axis one-shot (1 = still timing, 0 = done) |
| 2   | Joystick 2 X-axis one-shot (1 = still timing, 0 = done) |
| 1   | Joystick 1 Y-axis one-shot (1 = still timing, 0 = done) |
| 0   | Joystick 1 X-axis one-shot (1 = still timing, 0 = done) |

Timing: each axis one-shot bit stays high for roughly `axis_value * 11µs`.
CPU cycles until bit clears ≈ `axis_value * clock_speed / 90_909`.
Axis value 0–255 maps to centered (128) and full deflection (0 / 255).

Without an active gilrs poll, all axis bits return 0 (timing done) and all button bits return 1 (not pressed),
which is the correct "no joystick attached" response.

---

## Files to Create

### `core/src/devices/joystick.rs`

```rust
pub struct JoystickDevice {
    axes: [u8; 4],       // x1, y1, x2, y2 — 0..=255, center=128
    buttons: u8,         // bits 4-7 of status; 0=pressed (inverted)
    reset_cycle: u32,    // cycle_count when 0x201 was last written
    clock_speed: u32,    // CPU clock Hz for timing calc
}
```

- Implements `Device` for port `0x201`
- `io_write_u8(0x201, _, cycle_count)`: sets `reset_cycle = cycle_count`, returns `true`
- `io_read_u8(0x201, cycle_count)`:
  - `elapsed = cycle_count.saturating_sub(reset_cycle)`
  - For each axis `i`: `cycles_needed = axis[i] as u32 * clock_speed / 90_909`
  - Timing bit = `(elapsed < cycles_needed) as u8`
  - Returns `Some(timing_bits | button_bits)`
- Public setters: `set_axes(x1, y1, x2, y2: u8)`, `set_buttons(b1, b2, b3, b4: bool)`
- Constructor takes `clock_speed: u32`

### `native-common/src/gilrs_joystick.rs`

```rust
pub struct GilrsJoystick {
    gilrs: gilrs::Gilrs,
    joystick: Rc<RefCell<JoystickDevice>>,
    gamepad_id: Option<gilrs::GamepadId>,
    axes: [f32; 4],
    buttons: [bool; 4],
}
```

- `GilrsJoystick::new(joystick: Rc<RefCell<JoystickDevice>>) -> Option<Self>`
  - Returns `None` if gilrs fails to initialize (graceful degradation)
- `poll(&mut self)`: drains gilrs event queue, maps events to `JoystickDevice`:
  - Axis events → `set_axes()` (normalize −1.0..1.0 → 0..255, center=128)
  - Button events → `set_buttons()`
  - `Connected` event → captures first `GamepadId` seen
  - Ignores events from other gamepads

---

## Files to Modify

### `core/src/devices/mod.rs`
Add: `pub mod joystick;`

### `core/src/bus.rs`
- Add field `joystick: Rc<RefCell<JoystickDevice>>` to `Bus`
- In `Bus::new(clock_speed: u32, ...)`: `let joystick = Rc::new(RefCell::new(JoystickDevice::new(clock_speed)));`
- Add joystick to `devices` vec
- Add accessor: `pub fn joystick(&self) -> Rc<RefCell<JoystickDevice>>`

### `core/src/computer.rs`
- Add: `pub fn joystick(&self) -> Rc<RefCell<JoystickDevice>>` — delegates to `bus.joystick()`

### `Cargo.toml` (workspace)
```toml
gilrs = "0.11"
```

### `native-common/Cargo.toml`
```toml
gilrs = { workspace = true }
```

### `native-common/src/lib.rs`
- Add `pub mod gilrs_joystick;`
- Import `GilrsJoystick` from `gilrs_joystick` module
- Update `create_computer` **signature** to return `Option<GilrsJoystick>`:
  ```rust
  pub fn create_computer(
      cli: &CommonCli,
      video_buffer: Arc<RwLock<VideoBuffer>>,
      serial_mouse: Option<Arc<RwLock<SerialMouse>>>,
  ) -> Result<(Computer, Option<MixerDeviceSink>, Option<GilrsJoystick>)>
  ```
- At the end of `create_computer`, after building the `Computer`:
  ```rust
  let gilrs_joystick = if cli.joystick {
      GilrsJoystick::new(computer.joystick())
  } else {
      None
  };
  Ok((computer, sink, gilrs_joystick))
  ```

### `native-common/src/cli.rs`
```rust
/// Enable joystick/gamepad input on game port (0x201). Uses the first connected gamepad.
#[arg(long, default_value_t = false)]
pub joystick: bool,
```

### `native-gui/src/main.rs`
- Update `create_computer` call to destructure the third value:
  ```rust
  let (mut computer, audio_sink, mut gilrs_joystick) = create_computer(...)?;
  ```
- Replace the TODO comment at line 267–271 with:
  ```rust
  if let Some(ref mut js) = gilrs_joystick {
      js.poll();
  }
  ```

### `native-cli/src/main.rs`
- Update `create_computer` call to destructure the third value:
  ```rust
  let (mut computer, audio_sink, mut gilrs_joystick) = create_computer(...)?;
  ```
- In the emulation loop, before each step:
  ```rust
  if let Some(ref mut js) = gilrs_joystick { js.poll(); }
  ```

---

## Gilrs Axis Mapping

| gilrs Axis | Gameport axis | Index |
|------------|---------------|-------|
| `LeftStickX` | Joystick 1 X | 0 |
| `LeftStickY` | Joystick 1 Y | 1 |
| `RightStickX` | Joystick 2 X | 2 |
| `RightStickY` | Joystick 2 Y | 3 |

| gilrs Button | Gameport button |
|--------------|-----------------|
| `South` | Button 1 |
| `East` | Button 2 |
| `North` | Button 3 |
| `West` | Button 4 |

Normalize: `((axis_value + 1.0) * 127.5) as u8` (clamp 0..=255)

---

## Verification

1. Run `./scripts/pre-commit.sh` — must compile cleanly with no clippy warnings
2. Run `cargo test --all` — existing tests must pass; add a unit test for `JoystickDevice`:
   - Write 0x201 at cycle 0 with axis x1=128 (center), read at cycle 0 → bit 0 should be 1
   - Read after `128 * clock_speed / 90_909` cycles → bit 0 should be 0
   - Button pressed → corresponding bit should be 0 in status byte
3. Run `native-gui` with `--joystick` and a gamepad connected; run a DOS game that uses joystick calibration
4. Run without `--joystick` to confirm game port returns `0xFF` (no joystick attached behavior)
