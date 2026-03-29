# WASM UI Implementation Plan

## Overview

Build out the browser-based emulator UI in two parallel tracks:
1. **Rust WASM bridge** (`wasm/src/lib.rs`) — expose the core emulator to JavaScript
2. **React UI** (`wasm/www/src/`) — components for control, configuration, and display

---

## ✅ Phase 1: WASM Bridge (`wasm/src/lib.rs`)

`Oxide86Computer` is fully implemented.

### ✅ 1a. Config types (tsify)

`WasmComputerConfig` exported with `#[derive(Deserialize, Tsify)] #[tsify(from_wasm_abi)]`:

```rust
pub struct WasmComputerConfig {
    pub cpu_type: String,          // "8086" | "286" | "386" | "486"
    pub has_fpu: bool,
    pub memory_kb: u32,
    pub clock_hz: u32,             // 0 → defaults to 4,772,727 Hz
    pub video_card: String,        // "cga" | "ega" | "vga" | "mda" | "hgc"
    // Clock start date/time passed to EmulatedClock
    pub start_year: u16,
    pub start_month: u8,
    pub start_day: u8,
    pub start_hour: u8,
    pub start_minute: u8,
    pub start_second: u8,
}
```

### ✅ 1b. Constructor & lifecycle

```rust
impl Oxide86Computer {
    pub fn new(config: WasmComputerConfig) -> Result<Self, JsValue>
    pub fn power_on(&mut self, hdd_image: Option<Uint8Array>, boot_floppy: Option<Uint8Array>)
    pub fn power_off(&mut self)
    pub fn reboot(&mut self)
}
```

`new` validates config and sets up `wasm_logger` + `console_error_panic_hook`. `power_on`
creates the `Computer` (deferred from `new` so the HDD image can be included in the config),
inserts drives, and boots. `reboot` recreates the computer with cached disk images.

`set_hdd_image` was dropped — HDD is passed only via `power_on`; callers wanting a new HDD
call `power_on` again.

### ✅ 1c. Execution loop

```rust
pub fn run_for_cycles(&mut self, cycles: u32) -> RunResult
// RunResult: { halted: bool, exit_code: Option<u8>, cycles_executed: u32 }
```

Called from the JS `requestAnimationFrame` loop. Returning `halted` lets the UI update status.

### ✅1d. Video rendering

```rust
pub fn render_frame(&self) -> RenderResult
// RenderResult: { data: Vec<u8> (RGBA), width: u32, height: u32 }
```

The JS side copies this into a `Canvas` via `ImageData`. When powered off, returns a blank
640×400 frame.

### ✅ 1e. Input

```rust
pub fn push_key_event(&mut self, scan_code: u8, is_down: bool)
pub fn push_mouse_event(&mut self, dx: i16, dy: i16, buttons: u8)
```

`push_key_event` ORs `0x80` onto the make code for break (key-up) events.
`push_mouse_event` clamps `i16 → i8` before forwarding to `push_ps2_mouse_event`.
Keyboard: map browser `KeyboardEvent.code` → PC XT scan codes in TypeScript before calling in.

### ✅ 1f. Disk management

```rust
pub fn insert_floppy(&mut self, drive: u8, image: Uint8Array)  // drive: 0=A, 1=B
pub fn eject_floppy(&mut self, drive: u8)
// set_hdd_image removed — pass HDD via power_on
```

Floppy images are cached so `reboot` can re-insert them. Both A: and B: are supported.

### ✅ 1g. Metrics & status

```rust
pub fn get_effective_mhz(&mut self) -> f64   // cycles since last call / elapsed ms → MHz
pub fn get_cycle_count(&self) -> f64         // total cycles (f64 for JS safe integer range)
pub fn get_last_error(&mut self) -> Option<String>
```

`get_effective_mhz` uses `js_sys::Date::now()` to measure wall-clock elapsed time.
`wasm_logger` + `console_error_panic_hook` active from first `new()` call.

### Core change

Added `MemBackend::from_data(data: Vec<u8>) -> Self` (`core/src/disk/mem_backend.rs`) so
JS `Uint8Array` data can be wrapped as an in-memory disk image.

---

## Phase 2: React UI Components

Tech stack already in place: Mantine 7, Preact Signals, SCSS modules.

### Layout

```
┌──────────────────────────────────────────────────────────────────┐
│                        (page background)                         │
│                                                                  │
│              ┌──────────────────────┐  [A:][B:][C:]  [⏻][↺]    │
│              │                      │                             │
│              │       Screen         │  ← icon toolbar to right  │
│              │      (canvas)        │    of screen, horizontal   │
│              │                      │    groups with gap         │
│              └──────────────────────┘                            │
│              │  Status · · · · MHz  │  ← StatusBar below screen │
│              └──────────────────────┘                            │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

- Screen is centered horizontally and vertically on the page.
- StatusBar sits directly below the screen (same width), showing status messages on the left and MHz on the right.
- A horizontal icon toolbar sits to the right of the screen, with icon groups separated by a horizontal gap:
  - **Group 1 — Drives**: Floppy A:, Floppy B:, Hard disk C: (each opens a file picker / eject popover)
  - **Group 2 — Power**: Power On, Reboot (future: Power Off)
  - Additional groups can be added (settings, etc.)
- No AppShell header/footer — layout is plain flexbox/CSS centering.

Target component tree:

```
App
├── Screen             ← <canvas> + keyboard/mouse capture
├── StatusBar          ← status message (left) + MHz (right), below Screen
├── Toolbar            ← icon buttons to the right of Screen
│   ├── DriveButton    ← per-drive icon + popover (file input + eject)
│   └── PowerButton    ← Power On / Reboot icons
└── ConfigDrawer       ← slides in from right (Mantine Drawer), opened from Toolbar
    ├── MachineConfig  ← CPU type, FPU, RAM, clock speed, video card
    ├── ComPortConfig  ← COM1/COM2 device type (none, loopback, …) [stub]
    └── JoystickConfig ← enable/disable, axis sensitivity [stub]
```

### ✅ 2a. State (`wasm/www/src/state.ts`)

All computer interactions go through `State` — components never touch `Oxide86Computer` directly.
Signals are private; components get read-only access via `ReadonlySignal<T>` getters. All
mutations (power, drives, config) are methods on `State`.

Public API:
- **Getters**: `computer`, `status`, `config`, `floppyA`, `floppyB`, `hdd`
- **Config**: `updateConfig(patch)`
- **Status**: `setStatus(message, error?)` — called by Screen on halt
- **Power**: `powerOn(): Promise<void>`, `powerOff()`, `reboot()`
- **Drives**: `insertFloppy(drive: 0|1, file): Promise<void>`, `ejectFloppy(drive: 0|1)`, `setHdd(file|null)`

`powerOn` constructs `Oxide86Computer`, reads disk `File` objects into `Uint8Array`, calls
`power_on`, then updates `computerSignal`. `insertFloppy` does the same async read then calls
`computer.insert_floppy` if running. Components never hold a computer reference.

### ✅ 2b. Screen component (`wasm/www/src/components/Screen.tsx`)

- `<canvas>` with `image-rendering: pixelated`; resizes to match `RenderResult.width/height` on first frame.
- `get computer()` and `setStatus()` added to `State` (also exports `Status` interface).
- RAF loop uses `useSignalEffect` (not `useEffect`) so it reruns reactively when `state.computer` changes.
  Arrow function `tick` (not a `function` declaration) so TypeScript narrows closed-over consts correctly.
- Keyboard: `keydown`/`keyup` on `window` → `KEY_MAP[e.code]` → `computer.push_key_event(scanCode, isDown)`.
- Mouse: `canvas.requestPointerLock()` on click; `pointerlockchange` toggles `mousemove` listener → `push_mouse_event`.
- `keycodes.ts` created alongside with full XT scan code table (make codes; break = make | 0x80 handled in Rust).

### ✅ 2c. Toolbar (`wasm/www/src/components/Toolbar.tsx`) + DriveButton / PowerButton

Horizontal strip of Bootstrap icon `ActionIcon` buttons to the right of `Screen`. Groups separated
by CSS `gap: 1.5rem`; buttons within a group by `gap: 0.25rem`.

Icons use `bootstrap-icons` CSS classes (`bi bi-floppy`, `bi bi-power`, etc.) — already imported
in `main.tsx`.

#### DriveButton (one per drive: A:, B:, C:)
- `ActionIcon` (filled when a file is loaded, subtle when empty) opens a Mantine `Popover`.
- Popover contains: filename or "Empty", hidden `<input type="file">`, "Load image…" button,
  "Eject" button (floppy only, disabled when empty).
- On load: calls `state.insertFloppy(drive, file)` or `state.setHdd(file)`.
- On eject: calls `state.ejectFloppy(drive)`.

#### PowerButtons
- **Power On** — `onClick: void state.powerOn()`. Disabled when running.
- **Reboot** — `onClick: state.reboot()`. Disabled when off.
- Both buttons use `useComputed(() => state.computer.value !== null)` to reactively track run state.

### ✅ 2d. StatusBar (`wasm/www/src/components/StatusBar.tsx`)

Thin bar directly below `Screen`, same width as the canvas:

- Add `get status(): ReadonlySignal<...>`, `get perf(): ReadonlySignal<number>`,
  `dismissError()`, and `setPerf(mhz)` to `State`.
- **Left**: status message from `state.status.value.message`; if `error` is set, show it in red
  with a click-to-dismiss handler (`state.dismissError()`).
- **Right**: reads `state.perf`; updated by a `setInterval` every 500 ms that calls
  `computer.get_effective_mhz()` and `state.setPerf(mhz)`. Displays as `"X.XX MHz"`.

### ✅ 2e. MachineConfig (`wasm/www/src/components/MachineConfig.tsx`)

Mantine form controls that call `state.updateConfig(patch)`. Disabled while `state.computer.value` is non-null (machine is running). Add `updateConfig(patch: Partial<WasmComputerConfig>)` to `State` if not already present.

- `Select` — CPU type: `8086`, `286`
- `Switch` — FPU (math coprocessor)
- `Select` — RAM: 256 KB, 512 KB, 640 KB
- `Select` — clock: 4.77 MHz, 8 MHz, 10 MHz; plus a `NumberInput` that unlocks on "Custom"
- `Select` — video card: CGA, EGA, VGA

### ✅ 2g. App wiring (`wasm/www/src/App.tsx`)

Replace the current stub with the new layout (no `AppShell` — plain flexbox):

```tsx
// Outer: full-viewport flex, centering the inner block
// Inner: flex-col — Screen on top, StatusBar below
// Toolbar: flex-col or flex-row to the right of the inner block
<div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh' }}>
  <div style={{ display: 'flex', flexDirection: 'column' }}>
    <Screen />
    <StatusBar />
  </div>
  <Toolbar />
</div>
<ConfigDrawer />
```

---

## ✅ Phase 3: Keyboard Scan Code Mapping

A TypeScript file `src/keycodes.ts` maps `KeyboardEvent.code` (e.g. `"KeyA"`, `"ArrowUp"`) to
XT scan codes. Reference: https://stanislavs.org/helppc/make_codes.html

Only the keys that make it to `computer.push_key_event()` need to be in this table — unknown codes
are silently dropped.

---

## Recommended Starting Point

**Start with the WASM bridge and Screen component** — they validate the whole pipeline before
building the rest of the UI:

1. ~~Implement `Oxide86Computer::new()` with a hardcoded default config (8086, CGA, 640 KB).~~ ✅
2. ~~Implement `run_for_cycles()` + `render_frame()`.~~ ✅
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
| `wasm/src/lib.rs` | ✅ Done |
| `core/src/disk/mem_backend.rs` | ✅ Done — added `from_data` |
| `wasm/www/src/state.ts` | ✅ Done — `State` class, private signals, expose as `ReadonlySignal` getters on demand |
| `wasm/www/src/components/Screen.tsx` | ✅ Done — canvas + `useSignalEffect` RAF loop, keyboard, mouse |
| `wasm/www/src/keycodes.ts` | ✅ Done — XT scan code table |
| `wasm/www/src/components/Toolbar.tsx` | 2c — icon toolbar (drive + power groups) |
| `wasm/www/src/components/StatusBar.tsx` | 2d — status + perf bar below screen |
| `wasm/www/src/components/MachineConfig.tsx` | 2e — config form |
| `wasm/www/src/components/ConfigDrawer.tsx` | 2f — drawer wrapper (MachineConfig + stubs) |
| `wasm/www/src/App.tsx` | 2g — flexbox layout: Screen+StatusBar centered, Toolbar to right |
