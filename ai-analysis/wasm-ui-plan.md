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

Target component tree:

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

### ✅ 2a. State (`wasm/www/src/state.ts`)

Shared signal store implemented as a `State` class with all signals private. Signals are
exposed as `ReadonlySignal<T>` getters on demand — add a getter to `State` only when a
component first needs it.

```ts
export class State {
    private readonly computerSignal = signal<Oxide86Computer | null>(null);
    private readonly statusSignal   = signal<{ message: string; error: string | null }>(...);
    private readonly configSignal   = signal<WasmComputerConfig>(defaultConfig());
    private readonly perfSignal     = signal<number>(0);
    private readonly floppyASignal  = signal<File | null>(null);
    private readonly floppyBSignal  = signal<File | null>(null);
    private readonly hddSignal      = signal<File | null>(null);
}
export const state = new State();
```

`defaultConfig()` returns a valid `WasmComputerConfig` (8086, EGA, 640 KB, 4.77 MHz,
current date/time from `new Date()`). When a component needs to read a signal, add a
public getter typed as `ReadonlySignal<T>` and a mutation method alongside it — never
export the raw mutable signal.

### 2b. Screen component (`wasm/www/src/components/Screen.tsx`)

Validates the full Rust → canvas pipeline.

- `<canvas>` with CSS `image-rendering: pixelated`; resize to match `RenderResult.width/height` on first frame.
- Add `get computer(): ReadonlySignal<Oxide86Computer | null>` to `State`; expose a
  `setStatus(message, error?)` mutation method.
- `useEffect` starts the RAF loop when `state.computer` is non-null, cancels it on cleanup:
  ```ts
  function tick() {
    const result = computer.run_for_cycles(100_000)
    const frame  = computer.render_frame()
    const rgba   = new Uint8ClampedArray(frame.data)
    ctx.putImageData(new ImageData(rgba, frame.width, frame.height), 0, 0)
    if (!result.halted) raf = requestAnimationFrame(tick)
    else state.setStatus('Halted')
  }
  ```
- Click on canvas → `canvas.requestPointerLock()` to capture mouse; `pointerlockchange` → `push_mouse_event`.
- `keydown`/`keyup` on `window` → look up scan code in `keycodes.ts` → `computer.push_key_event(scanCode, isDown)`.

### 2c. ControlPanel (`wasm/www/src/components/ControlPanel.tsx`)

Three Mantine `Button` components in a `Group`:

- Add `get config(): ReadonlySignal<WasmComputerConfig>`, `get floppyA/B/hdd()` getters and
  `setComputer(c)` mutation to `State`.
- **Power On** — reads `state.config` + disk signals, calls `new Oxide86Computer(config)`, then
  `power_on(hdd, floppy)`. Calls `state.setComputer(computer)` and `state.setStatus(...)`.
- **Power Off** — calls `computer.power_off()`, calls `state.setComputer(null)`.
- **Reboot** — calls `computer.reboot()`, triggers RAF restart via `state.setComputer(computer)`.

Disable Power Off / Reboot when `state.computer.value` is null; disable Power On when non-null.

### 2d. StatusBar (`wasm/www/src/components/StatusBar.tsx`)

Thin bar, always visible (position it at the bottom of the layout):

- Add `get status(): ReadonlySignal<...>`, `get perf(): ReadonlySignal<number>`,
  `dismissError()`, and `setPerf(mhz)` to `State`.
- **Left**: status message from `state.status.value.message`.
- **Center**: error from `state.status.value.error` — red Mantine `Text`, click to dismiss
  via `state.dismissError()`.
- **Right**: PerfBar — reads `state.perf`, updated by a `setInterval` every 500 ms that calls
  `computer.get_effective_mhz()` and calls `state.setPerf(mhz)`. Displays as `"X.XX MHz"`.

### 2e. MachineConfig (`wasm/www/src/components/MachineConfig.tsx`)

Mantine form controls that call `state.updateConfig(patch)`. Disabled while `state.computer.value` is non-null (machine is running). Add `updateConfig(patch: Partial<WasmComputerConfig>)` to `State` if not already present.

- `Select` — CPU type: `8086`, `286`
- `Switch` — FPU (math coprocessor)
- `Select` — RAM: 256 KB, 512 KB, 640 KB
- `Select` — clock: 4.77 MHz, 8 MHz, 10 MHz; plus a `NumberInput` that unlocks on "Custom"
- `Select` — video card: CGA, EGA, VGA
- Date/time pickers (or `NumberInput` fields) for `start_year/month/day/hour/minute/second`

### 2f. DriveManager (`wasm/www/src/components/DriveManager.tsx`)

One row per drive (Floppy A:, Floppy B:, Hard disk C:):

- File `<input type="file" accept=".img,.ima,.bin">` — on change: read as `ArrayBuffer`, wrap in `Uint8Array`.
  - Floppy: calls `computer.insert_floppy(drive, image)` if machine is running; always calls `state.setFloppyA/B(file)`.
  - HDD: calls `state.setHdd(file)` only (takes effect on next Power On).
- Eject button (floppy only) — calls `computer.eject_floppy(drive)`, calls `state.setFloppyA/B(null)`.
- Show filename of currently inserted image or "Empty".

### 2g. ConfigDrawer (`wasm/www/src/components/ConfigDrawer.tsx`)

Mantine `Drawer` opening from the right, triggered by a settings button in the layout:

- Contains `MachineConfig` and `DriveManager` as labelled sections (`Title` + divider).
- `ComPortConfig` and `JoystickConfig` panels are stubs (placeholders) for now.

### 2h. App wiring (`wasm/www/src/App.tsx`)

Replace the current stub with the full layout:

```tsx
<AppShell header={<ControlPanel />} footer={<StatusBar />}>
  <Screen />
  <ConfigDrawer />
</AppShell>
```

Wire the settings button (in `ControlPanel` or a toolbar) to open `ConfigDrawer`.

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
| `wasm/www/src/components/Screen.tsx` | 2b — canvas + RAF loop |
| `wasm/www/src/keycodes.ts` | 2b — scan code table (needed by Screen) |
| `wasm/www/src/components/ControlPanel.tsx` | 2c — power buttons |
| `wasm/www/src/components/StatusBar.tsx` | 2d — status + perf bar |
| `wasm/www/src/components/MachineConfig.tsx` | 2e — config form |
| `wasm/www/src/components/DriveManager.tsx` | 2f — disk image upload |
| `wasm/www/src/components/ConfigDrawer.tsx` | 2g — drawer wrapper |
| `wasm/www/src/App.tsx` | 2h — wire everything together |
