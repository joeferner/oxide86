# Plan: Migrate WASM React App to Preact Signals

## Background

The WASM front-end (`wasm/www/`) is a React + TypeScript application with ~13 components and 2 custom hooks.
State is managed via `useState`, `useEffect`, `useCallback`, and `useRef`.

A recurring pattern is using refs to escape stale closures inside animation frames and event handlers:
- `computerRef` — keeps current computer for gamepad handlers
- `isRunningRef` — tracks running state inside `requestAnimationFrame` loop
- `configRef` — avoids stale config inside WASM callbacks

Preact Signals (`@preact/signals-react`) solves stale closures structurally: reading `.value` inside any
callback always returns the current value without needing a ref shadow.

The goal is to replace `useState`/`useCallback`/`useEffect`-for-watching-state with signals and computed
values, while keeping DOM-ref usage (`useRef<HTMLCanvasElement>`) and one-time lifecycle effects (WASM init).

---

## Approach

Use `@preact/signals-react` (not a full switch to Preact). This package integrates with React and allows:

```ts
import { signal, computed, effect } from "@preact/signals-react";
```

Components auto-subscribe when they read `signal.value` in render — no extra wiring needed.

---

## Files to Modify

### Dependencies

| File | Change |
|------|--------|
| `wasm/www/package.json` | Add `@preact/signals-react` |

### Phase 1 — Core emulator state (`useEmulator.ts` → `emulatorState.ts`)

This is the highest-value conversion. The stale-closure refs disappear entirely.

| Current | Replacement |
|---------|-------------|
| `useState<Emu86Computer \| null>` | `signal<Emu86Computer \| null>(null)` |
| `useState<string>` (status) | `signal<string>("")` |
| `useState<boolean>` (isRunning) | `signal<boolean>(false)` |
| `useState<Performance>` | `signal<Performance>(...)` |
| `useRef` computerRef | **deleted** — read `computer.value` directly |
| `useRef` isRunningRef | **deleted** — read `isRunning.value` directly |
| `useRef` configRef | **deleted** — read `config.value` directly |
| `useRef` gamepadSlotsRef | Keep as `useRef<Map>` (not reactive state) |
| `useRef` animationFrameRef | Keep as `useRef<number \| null>` (imperative handle) |
| `useRef` wasmInitializedRef | Keep as `useRef<boolean>` (one-time init guard) |

The hook becomes a thin module that exports signals and action functions. It no longer needs to be a
React hook at all since it has no DOM dependency — it can be a plain module with top-level signals.
Components import signals directly instead of receiving them as props.

```ts
// emulatorState.ts
export const computer = signal<Emu86Computer | null>(null);
export const status = signal("");
export const isRunning = signal(false);
export const performance = signal<Performance>({ target: 4.77, actual: 0 });
export const config = signal<EmulatorConfig>(loadConfig());

// actions (plain async functions, no hooks)
export async function initEmulator(canvas: HTMLCanvasElement) { ... }
export function startExecution() { ... }
export function stopExecution() { ... }
export function reset() { ... }
```

Gamepad event handlers and the `requestAnimationFrame` loop both read `computer.value` / `isRunning.value`
inline — no refs needed.

### Phase 2 — App.tsx

| Current | Replacement |
|---------|-------------|
| `useState<'boot'\|'program'>` (mode) | `signal` |
| `useState<boolean>` × 4 (dialog open flags) | `signal` × 4 |
| `useState<number>` (selectedDrive, bootDrive) | `signal` × 2 |
| `useState<boolean>` (hasBooted) | `signal` |
| `useState<[string\|null,string\|null]>` (floppyLabels) | `signal` |
| All `useCallback` handlers | Plain functions (signals avoid stale closures) |
| `useEffect` beforeUnload listener | Keep as `useEffect` (DOM side-effect) |

With emulator state in a module, `App.tsx` stops being the state container and becomes a layout
component that reads signals from `emulatorState.ts` directly.

### Phase 3 — Component local state

Straightforward conversions. Each component-local signal is defined inside the component function
(same scoping as `useState`) or lifted to the module level if shared.

| Component | useState count | Notes |
|-----------|---------------|-------|
| `DriveControl.tsx` | 3 | floppyAFile, floppyBFile, hddFile — keep local |
| `ConfigDialog.tsx` | 8 | Form state — keep local to ConfigForm |
| `ProgramControl.tsx` | 3 | file, segment, offset — keep local |
| `DiskManager.tsx` | 7 | currentPath, files, loading, modals — keep local |

The `useEffect([computer])` remount in `DriveControl` becomes an `effect(() => { ... })` that
automatically re-runs whenever `computer.value` changes.

The `browseDisk` useCallback in `DiskManager` becomes a plain `async function` — no dependency array.

### Phase 4 — usePointerLock

Convert from a React hook to a plain signals module (no JSX, no DOM ref dependency beyond canvas).

```ts
// pointerLockState.ts
export const isLocked = signal(false);
export function requestLock(canvas: HTMLCanvasElement) { ... }
export function exitLock() { ... }
// effect() replaces useEffect for pointerlockchange listener
```

`EmulatorCanvas.tsx` imports `isLocked` directly and reads `isLocked.value`.

### Phase 5 — Presentational components

**No changes needed.** BootControl, ExecutionControl, StatusDisplay, RunningIndicator,
PerformanceDisplay, InfoBox are pure render functions with no state.

They currently receive values as props. After migration they can either:
- Continue receiving props (no change), or
- Import signals directly (removes prop drilling for deeply nested ones)

---

## Implementation Order

1. **Install package** — `npm install @preact/signals-react` in `wasm/www/`
2. **Phase 1** — Create `emulatorState.ts`, migrate `useEmulator.ts` logic into it, delete the hook
3. **Phase 2** — Rewrite `App.tsx` to import from `emulatorState.ts`, convert local state to signals
4. **Phase 3** — Convert `DriveControl`, `DiskManager`, `ConfigDialog`, `ProgramControl`
5. **Phase 4** — Create `pointerLockState.ts`, update `EmulatorCanvas.tsx`
6. **Cleanup** — Delete `hooks/useEmulator.ts` and `hooks/usePointerLock.ts`, verify no leftover hook imports

---

## Critical Details

### WASM initialization (must-not-break)

The one-time init guard in `useEmulator` currently uses `wasmInitializedRef`. This **stays as a ref**
(not a signal) because it guards imperative side-effectful code that must run exactly once, not reactive state:

```ts
const wasmInitializedRef = useRef(false); // stays
if (!wasmInitializedRef.current) {
    wasmInitializedRef.current = true;
    await init(); // wasm-bindgen init
}
```

### Animation frame loop

The RAF loop reads `isRunning.value` and `computer.value` directly — no refs:

```ts
function loop() {
    if (!isRunning.value || !computer.value) return;
    computer.value.step_frame();
    animationFrameRef.current = requestAnimationFrame(loop);
}
```

### Gamepad event handlers

Currently use `computerRef.current` to avoid stale closure. After migration:

```ts
window.addEventListener("gamepadconnected", (e) => {
    const comp = computer.value; // always current
    if (!comp) return;
    ...
});
```

### DriveControl remount

Currently:
```ts
useEffect(() => {
    // remount drives when computer changes
}, [computer]); // eslint-disable-next-line react-hooks/exhaustive-deps
```

After migration:
```ts
effect(() => {
    const comp = computer.value; // auto-tracks dependency
    if (!comp) return;
    // remount drives
});
```

The eslint-disable comment and the dependency array footgun both disappear.

---

## Files Summary

| File | Action |
|------|--------|
| `wasm/www/package.json` | Add dependency |
| `wasm/www/src/hooks/useEmulator.ts` | **Delete** → replaced by `emulatorState.ts` |
| `wasm/www/src/hooks/usePointerLock.ts` | **Delete** → replaced by `pointerLockState.ts` |
| `wasm/www/src/emulatorState.ts` | **New** — signals + actions for emulator lifecycle |
| `wasm/www/src/pointerLockState.ts` | **New** — signals + actions for pointer lock |
| `wasm/www/src/App.tsx` | Rewrite state wiring |
| `wasm/www/src/components/EmulatorCanvas.tsx` | Use pointerLockState signals |
| `wasm/www/src/components/DriveControl.tsx` | Convert local state + remount effect |
| `wasm/www/src/components/DiskManager.tsx` | Convert local state + remove useCallback |
| `wasm/www/src/components/ConfigDialog.tsx` | Convert local form state |
| `wasm/www/src/components/ProgramControl.tsx` | Convert local state |
| `wasm/www/src/components/BootControl.tsx` | No change |
| `wasm/www/src/components/ExecutionControl.tsx` | No change |
| `wasm/www/src/components/StatusDisplay.tsx` | No change |
| `wasm/www/src/components/RunningIndicator.tsx` | No change |
| `wasm/www/src/components/PerformanceDisplay.tsx` | No change |
| `wasm/www/src/components/InfoBox.tsx` | No change |
