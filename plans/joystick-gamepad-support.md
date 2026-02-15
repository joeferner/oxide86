# Plan: Joystick/Gamepad Support

## Background

PC joysticks connect via the **IBM Game Control Adapter** (port `0x201`). Up to two joysticks (A and B) can be connected, each with 2 analog axes (X/Y) and 2 buttons.

### Hardware Protocol (Port 0x201)

**Write** any value to `0x201`: fires all four RC-timer one-shots simultaneously — all axis bits go high.

**Read** `0x201`:
```
Bit 0 — Joystick A X-axis timer (1=not timed out, 0=timed out)
Bit 1 — Joystick A Y-axis timer
Bit 2 — Joystick B X-axis timer
Bit 3 — Joystick B Y-axis timer
Bit 4 — Joystick A Button 1 (0=pressed, 1=released)
Bit 5 — Joystick A Button 2
Bit 6 — Joystick B Button 1
Bit 7 — Joystick B Button 2
```

**Axis emulation**: Programs write to `0x201`, then loop counting reads until each axis bit drops to 0. The count represents axis position. For emulation at 4.77 MHz:
- Axis range: ~115–3860 cycles (0%–100% deflection, ~24µs–816µs)
- Center: ~2000 cycles (~420µs)
- Formula: `timeout_cycles = 115 + axis_value_0_to_1 * 3745`

---

## Files to Create / Modify

### New Files

| File | Purpose |
|------|---------|
| `core/src/joystick.rs` | `JoystickInput` trait + `JoystickState` + `NullJoystick` |
| `core/src/io/joystick_port.rs` | Port `0x201` timing emulation (`JoystickPort`) |
| `native-cli/src/terminal_joystick.rs` | `NullJoystick` alias / thin wrapper for CLI |
| `native-gui/src/gui_joystick.rs` | `GuiJoystick` using `gilrs` crate |
| `wasm/src/web_joystick.rs` | `WebJoystick` using Gamepad API |
| `test-programs/joystick/joystick_test.asm` | Test program showing axis/button state |
| `test-programs/joystick/README.md` | Instructions for running the test |

### Modified Files

| File | Change |
|------|--------|
| `core/src/lib.rs` | Add `joystick` module; pass `JoystickInput` to `Computer::new()` |
| `core/src/computer.rs` | Call `joystick_port.update()` each step; sync cycles |
| `core/src/io/mod.rs` | Route port `0x201` to `JoystickPort` |
| `core/src/cpu/bios/mod.rs` | Pass `JoystickInput` into `Bios` |
| `native-cli/src/main.rs` | `--joystick-a` / `--joystick-b` args; construct joystick |
| `native-gui/src/main.rs` | Same CLI args; wire gilrs events → `GuiJoystick` |
| `wasm/src/lib.rs` | `joystick_connected(slot, enabled)` + `handle_gamepad_axis/button` methods |
| `wasm/www/pkg/emu86_wasm.d.ts` | Add `handle_gamepad_axis`, `handle_gamepad_button`, `ComputerConfig.joystick_a/b` |
| `native-gui/Cargo.toml` | Add `gilrs` optional dependency |
| `test-programs/README.md` | Add joystick test entry |

---

## Step-by-Step Implementation

### Step 1 — `JoystickInput` Trait (`core/src/joystick.rs`)

```rust
pub trait JoystickInput: Send {
    /// Normalized axis values: 0.0 = full left/up, 1.0 = full right/down, 0.5 = center
    fn get_axis(&self, joystick: u8, axis: u8) -> f32;  // joystick 0/1, axis 0=X 1=Y
    fn get_button(&self, joystick: u8, button: u8) -> bool;  // button 0/1
    fn is_connected(&self, joystick: u8) -> bool;
}

pub struct JoystickState {
    pub x: f32,       // 0.0–1.0
    pub y: f32,
    pub button1: bool,
    pub button2: bool,
    pub connected: bool,
}

/// NullJoystick: no joystick connected
pub struct NullJoystick;
impl JoystickInput for NullJoystick {
    fn get_axis(&self, _j: u8, _a: u8) -> f32 { 0.5 }
    fn get_button(&self, _j: u8, _b: u8) -> bool { false }
    fn is_connected(&self, _j: u8) -> bool { false }
}
```

Both joysticks A and B are served by the **same** trait implementation — the implementation holds state for both slots (e.g., `[JoystickState; 2]`).

---

### Step 2 — Port 0x201 Emulation (`core/src/io/joystick_port.rs`)

```rust
pub struct JoystickPort {
    joystick: Box<dyn JoystickInput>,
    fire_cycle: Option<u64>,   // cycle count when last write fired one-shots
}

const MIN_CYCLES: u64 = 115;
const MAX_CYCLES: u64 = 3860;

impl JoystickPort {
    pub fn fire(&mut self, current_cycle: u64) {
        self.fire_cycle = Some(current_cycle);
    }

    pub fn read(&self, current_cycle: u64) -> u8 {
        let mut result = 0u8;

        if let Some(fire) = self.fire_cycle {
            let elapsed = current_cycle.saturating_sub(fire);

            for j in 0..2u8 {
                for a in 0..2u8 {
                    let axis_val = self.joystick.get_axis(j, a);
                    let timeout = MIN_CYCLES + (axis_val * (MAX_CYCLES - MIN_CYCLES) as f32) as u64;
                    let bit = j * 2 + a;      // bits 0-3
                    if elapsed < timeout && self.joystick.is_connected(j) {
                        result |= 1 << bit;   // timer still running
                    }
                }
            }
        }

        // Button bits 4-7 (0=pressed)
        for j in 0..2u8 {
            for b in 0..2u8 {
                let bit = 4 + j * 2 + b;
                if !self.joystick.get_button(j, b) || !self.joystick.is_connected(j) {
                    result |= 1 << bit;  // not pressed
                }
            }
        }

        result
    }
}
```

---

### Step 3 — Wire into I/O (`core/src/io/mod.rs`)

Add `JoystickPort` to `IoDevice`:
```rust
pub struct IoDevice {
    // ... existing fields
    pub joystick: JoystickPort,
}
```

In `read_byte`:
```rust
0x201 => self.joystick.read(self.current_cycle),
```

In `write_byte`:
```rust
0x201 => self.joystick.fire(self.current_cycle),
```

`IoDevice` needs a `current_cycle: u64` field updated each step from `Computer`.

---

### Step 4 — Native GUI (`native-gui/src/gui_joystick.rs`)

Use the [`gilrs`](https://crates.io/crates/gilrs) crate (cross-platform gamepad library):

```rust
use gilrs::{Gilrs, Event, EventType, Axis, Button};
use std::sync::{Arc, Mutex};

pub struct GuiJoystick {
    state: Arc<Mutex<[JoystickState; 2]>>,
    gilrs: Gilrs,
}

impl GuiJoystick {
    pub fn new() -> Self { ... }
    pub fn poll(&mut self) { /* drain gilrs events, update state */ }
}
```

The main GUI event loop calls `gui_joystick.poll()` on each frame.

**Axis mapping** (gilrs `Axis` → joystick slot):
- First connected gamepad → slot 0 (A)
- Second connected gamepad → slot 1 (B)
- `LeftStickX` / `LeftStickY` → X/Y axes
- `South` (button A) → button 1, `East` (button B) → button 2

---

### Step 5 — WASM (`wasm/src/web_joystick.rs`)

Use the browser [Gamepad API](https://developer.mozilla.org/en-US/docs/Web/API/Gamepad_API):

```rust
pub struct WebJoystick {
    states: [JoystickState; 2],
}

impl WebJoystick {
    pub fn update_axis(&mut self, slot: u8, axis: u8, value: f32) { ... }
    pub fn update_button(&mut self, slot: u8, button: u8, pressed: bool) { ... }
    pub fn set_connected(&mut self, slot: u8, connected: bool) { ... }
}
```

WASM public API additions to `Emu86Computer`:

```typescript
// In ComputerConfig
joystick_a?: boolean;   // whether to enable joystick A slot
joystick_b?: boolean;   // whether to enable joystick B slot

// On Emu86Computer instance
handle_gamepad_axis(slot: number, axis: number, value: number): void;
handle_gamepad_button(slot: number, button: number, pressed: boolean): void;
gamepad_connected(slot: number, connected: boolean): void;
```

The JavaScript/TypeScript side polls `navigator.getGamepads()` each animation frame and calls these methods. The WASM module does not call into the Gamepad API directly.

---

### Step 6 — CLI Configuration (`native-cli/src/main.rs`)

Add `clap` arguments:
```
--joystick-a          Enable joystick A (uses NullJoystick, no hardware input in CLI)
--joystick-b          Enable joystick B
```

In CLI, joystick slots are always `NullJoystick` (axes fixed at 0.5 center, no buttons) but the `is_connected()` flag responds to the CLI flags.

---

### Step 7 — Test Program (`test-programs/joystick/joystick_test.asm`)

NASM `.COM` program that:
1. Reads port `0x201` and prints axis timer counts and button states to screen in a loop
2. Exits on any key press

```nasm
; joystick_test.com - Reads joystick port 0x201 and displays state
; Usage: Run with joystick connected (or in emulator with joystick enabled)
org 0x100

main:
    mov ah, 0x01            ; check for keystroke
    int 0x16
    jnz exit

    ; Fire one-shots
    out 0x201, al

    ; Small delay
    mov cx, 1000
.delay:
    loop .delay

    ; Read result
    in al, 0x201

    ; Display bits (axes and buttons)
    ; ... (print hex byte to screen using INT 10h)

    jmp main
exit:
    mov ax, 0x4C00
    int 0x21
```

The test program will display the raw `0x201` byte as two hex digits, allowing verification that:
- Without joystick: all `FF` (axes timed out immediately since `is_connected()` = false)
- With gamepad plugged in (GUI): axes decrease over time, buttons change on press

---

## Configuration Summary

### Native CLI
```bash
# No joystick (default)
cargo run -p emu86-native-cli -- program.com

# Enable joystick A slot (NullJoystick - always at center, no buttons)
cargo run -p emu86-native-cli -- --joystick-a program.com
```

### Native GUI
```bash
# Auto-detect via gilrs (if any gamepad is plugged in, it maps to slot A/B automatically)
cargo run -p emu86-native-gui -- program.com

# Disable joystick detection
cargo run -p emu86-native-gui -- --no-joystick program.com
```

### WASM
```javascript
const computer = new Emu86Computer({
    canvas_id: "canvas",
    joystick_a: true,   // enable slot A
    joystick_b: false,  // slot B disabled
});

// Poll Gamepad API in animation frame:
function gamepadPoll() {
    const pads = navigator.getGamepads();
    if (pads[0]) {
        computer.handle_gamepad_axis(0, 0, (pads[0].axes[0] + 1) / 2);  // normalize -1..1 → 0..1
        computer.handle_gamepad_axis(0, 1, (pads[0].axes[1] + 1) / 2);
        computer.handle_gamepad_button(0, 0, pads[0].buttons[0].pressed);
        computer.handle_gamepad_button(0, 1, pads[0].buttons[1].pressed);
    }
    requestAnimationFrame(gamepadPoll);
}
```

---

## Cargo.toml Changes

### `native-gui/Cargo.toml`
```toml
[dependencies]
gilrs = { version = "0.10", optional = true }

[features]
default = ["gamepad"]
gamepad = ["gilrs"]
```

No new WASM dependencies needed (uses browser Gamepad API via `web-sys`/`js-sys`).

### `core/Cargo.toml`
No new dependencies — joystick trait is pure Rust.

---

## Implementation Order

1. `core/src/joystick.rs` — trait + NullJoystick
2. `core/src/io/joystick_port.rs` — port 0x201 logic
3. Wire into `core/src/io/mod.rs` and `core/src/computer.rs`
4. `native-cli` — NullJoystick + CLI flag
5. `native-gui` — GuiJoystick with gilrs
6. `wasm` — WebJoystick + WASM public API
7. Test program + README update
8. Run `./scripts/pre-commit.sh`