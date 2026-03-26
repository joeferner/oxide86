# WASM UI Implementation Plan

## Overview

Build out the browser-based emulator UI in two parallel tracks:
1. **Rust WASM bridge** (`wasm/src/lib.rs`) — expose the core emulator to JavaScript
2. **React UI** (`wasm/www/src/`) — components for control, configuration, and display

---

## Phase 1: WASM Bridge (`wasm/src/lib.rs`)

The current `Oxide86Computer` struct is a stub. This phase implements the full Rust→JS API.

### 1a. Config types (tsify)

Export a `WasmComputerConfig` struct with `#[derive(Deserialize, Tsify)]` so TypeScript
gets auto-generated types:

```rust
pub struct WasmComputerConfig {
    pub cpu_type: String,          // "8086" | "286"
    pub has_fpu: bool,
    pub memory_kb: u32,
    pub clock_hz: u32,
    pub video_card: String,        // "cga" | "ega" | "vga"
}
```

Maps to `ComputerConfig` from the core crate.

### 1b. Constructor & lifecycle

```rust
impl Oxide86Computer {
    pub fn new(config: WasmComputerConfig) -> Result<Self, JsValue>
    pub fn power_on(&mut self, hdd_image: Option<Uint8Array>, boot_floppy: Option<Uint8Array>)
    pub fn power_off(&mut self)
    pub fn reboot(&mut self)
}
```

### 1c. Execution loop

```rust
pub fn run_for_cycles(&mut self, cycles: u32) -> RunResult
// RunResult: { halted: bool, exit_code: Option<u8>, cycles_executed: u32 }
```

Called from the JS `requestAnimationFrame` loop. Returning `halted` lets the UI update status.

### 1d. Video rendering

```rust
pub fn render_frame(&mut self) -> RenderResult
// RenderResult: { data: Vec<u8> (RGBA), width: u32, height: u32 }
```

The JS side copies this into a `Canvas` via `ImageData`.

### 1e. Input

```rust
pub fn push_key_event(&mut self, scan_code: u8, is_down: bool)
pub fn push_mouse_event(&mut self, dx: i16, dy: i16, buttons: u8)
```

Keyboard: map browser `KeyboardEvent.code` → PC XT scan codes in TypeScript before calling in.

### 1f. Disk management

```rust
pub fn insert_floppy(&mut self, drive: u8, image: Uint8Array)  // drive: 0=A, 1=B
pub fn eject_floppy(&mut self, drive: u8)
pub fn set_hdd_image(&mut self, image: Uint8Array)
```

### 1g. Metrics & status

```rust
pub fn get_effective_mhz(&self) -> f64     // cycles since last call / elapsed ms → MHz
pub fn get_cycle_count(&self) -> f64       // total cycles (f64 for JS safe integer range)
pub fn get_last_error(&self) -> Option<String>
```

Use `wasm_logger` + `console_error_panic_hook` (already in Cargo.toml) for browser console output.

---

## Phase 2: React UI Components

Tech stack already in place: Mantine 7, Preact Signals, SCSS modules.

### Component tree

```
App
├── StatusBar          ← last message / last error / CPU MHz
├── Screen             ← <canvas> + keyboard/mouse capture
├── ControlPanel       ← Power On / Power Off / Reboot
├── ConfigDrawer       ← slides in from right (Mantine Drawer)
│   ├── MachineConfig  ← CPU type, FPU, RAM, clock speed, video card
│   ├── DriveManager   ← floppy A/B file upload + eject; HDD file upload
│   ├── ComPortConfig  ← COM1/COM2 device type (none, loopback, …)
│   └── JoystickConfig ← enable/disable, axis sensitivity
└── PerfBar            ← live MHz gauge (updates every ~500 ms)
```

### State management (Preact Signals)

```ts
const computerSignal = signal<Oxide86Computer | null>(null)
const statusSignal   = signal<{ message: string; error: string | null }>({ message: "Off", error: null })
const configSignal   = signal<WasmComputerConfig>(defaultConfig)
const perfSignal     = signal<number>(0)   // MHz
```

### Screen component

- `<canvas>` sized to the emulator's render resolution (320×200 scaled up with CSS `image-rendering: pixelated`).
- `useEffect` sets up the `requestAnimationFrame` loop:
  ```ts
  function tick() {
    const result = computer.run_for_cycles(100_000)
    const frame  = computer.render_frame()
    ctx.putImageData(new ImageData(frame.data, frame.width, frame.height), 0, 0)
    if (!result.halted) raf = requestAnimationFrame(tick)
    else statusSignal.value = { message: "Halted", error: null }
  }
  ```
- Click on canvas → `canvas.requestPointerLock()` to capture mouse; Escape to release.
- `keydown`/`keyup` on `window` while canvas is focused → `computer.push_key_event(scanCode, isDown)`.

### ControlPanel

Three Mantine `Button` components:
- **Power On** — builds `ComputerConfig` from `configSignal`, calls `Oxide86Computer.new()`, loads disk images, calls `power_on()`, starts the RAF loop.
- **Power Off** — stops RAF loop, calls `power_off()`, frees the WASM object.
- **Reboot** — calls `reboot()`, restarts RAF loop.

### DriveManager

- File `<input type="file" accept=".img,.ima,.bin">` for each drive.
- On change: read file as `ArrayBuffer`, wrap in `Uint8Array`, call `insert_floppy` / `set_hdd_image`.
- Eject button calls `eject_floppy`.
- Show drive label + filename of currently inserted image.

### MachineConfig

Mantine form controls:
- `Select` for CPU type: `8086`, `286`
- `Switch` for FPU (math coprocessor)
- `Select` or `NumberInput` for RAM: 256 KB, 512 KB, 640 KB
- `Select` for clock speed: 4.77 MHz, 8 MHz, 10 MHz, custom
- `Select` for video card: CGA, EGA, VGA

### StatusBar

- Thin bar at the bottom, always visible.
- Left: status message (Computer started, Computer stopped, Halted, etc.)
- Center: last error (red text, dismissible)
- Right: `PerfBar` — current effective MHz, updates on a 500 ms `setInterval` that reads `perfSignal`.

---

## Phase 3: Keyboard Scan Code Mapping

A TypeScript file `src/keycodes.ts` maps `KeyboardEvent.code` (e.g. `"KeyA"`, `"ArrowUp"`) to
XT scan codes. Reference: https://stanislavs.org/helppc/make_codes.html

Only the keys that make it to `computer.push_key_event()` need to be in this table — unknown codes
are silently dropped.

---

## Recommended Starting Point

**Start with the WASM bridge and Screen component** — they validate the whole pipeline before
building the rest of the UI:

1. Implement `Oxide86Computer::new()` with a hardcoded default config (8086, CGA, 640 KB).
2. Implement `run_for_cycles()` + `render_frame()`.
3. Build the `Screen` component with the RAF loop and canvas rendering.
4. Confirm you can see the emulator running in the browser.

Then layer in:
5. `push_key_event` + keyboard mapping (so you can type).
6. `ControlPanel` (power/reboot) + `StatusBar`.
7. `DriveManager` (floppy/HDD image upload).
8. `ConfigDrawer` (CPU config, COM ports, joystick).
9. `PerfBar` + metrics.

---

## Files to Create / Modify

| File | Action |
|---|---|
| `wasm/src/lib.rs` | Full rewrite — implement all bridge methods |
| `wasm/www/src/App.tsx` | Replace stub with component tree |
| `wasm/www/src/components/Screen.tsx` | New — canvas + RAF loop |
| `wasm/www/src/components/ControlPanel.tsx` | New — power buttons |
| `wasm/www/src/components/StatusBar.tsx` | New — status + perf |
| `wasm/www/src/components/ConfigDrawer.tsx` | New — config panels |
| `wasm/www/src/components/DriveManager.tsx` | New — disk image upload |
| `wasm/www/src/keycodes.ts` | New — scan code table |
| `wasm/www/src/state.ts` | New — Preact Signal declarations |
