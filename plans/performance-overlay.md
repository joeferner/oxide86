# Performance Overlay Implementation Plan

## Overview
Add performance monitoring to both native-gui and wasm:
- **native-gui**: Toggleable overlay in top-right corner showing target and actual clock rates (visible even in exclusive mode)
- **wasm**: Performance display next to the running indicator in HTML (always visible when running)

## Part 1: Native-GUI Implementation

### 1. Performance Tracking Structure

Create a `PerformanceTracker` struct that maintains a rolling window of performance measurements to calculate smooth, stable MHz readings:

```rust
struct PerformanceTracker {
    last_update_time: Instant,
    last_cycle_count: u64,
    current_mhz: f64,
    update_interval_ms: u64,
}
```

**Key methods:**
- `new()` - Initialize with current time and zero values
- `update(current_cycles: u64)` - Every 200ms, calculate instantaneous MHz and apply exponential moving average smoothing (0.7 old + 0.3 new)
- `get_mhz()` -> f64 - Return the smoothed MHz value

**Design rationale:** The 200ms update interval balances responsiveness with stability. Exponential moving average prevents jitter while staying responsive to real changes.

### 2. State Management

Add two fields to `AppState` struct (line 330 in [main.rs](native-gui/src/main.rs)):
- `show_performance_overlay: bool` - Toggle state
- `perf_tracker: PerformanceTracker` - Performance calculation

### 3. Overlay Rendering

Create `render_performance_overlay()` function that uses `egui::Window`:
- Anchor to `RIGHT_TOP` with 10px padding: `.anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 10.0))`
- Disable title bar, resizing, moving, collapsing
- Display two lines:
  - "Target: 4.77 MHz" (or "Unlimited" in turbo mode)
  - "Actual: X.XX MHz" (formatted to 2 decimal places)

Call this function in `process_egui_frame()` (line 388) **outside** the `!exclusive_mode` check so it remains visible during gameplay.

### 4. Menu Integration

**Add to [menu.rs](native-gui/src/menu.rs):**
- New `MenuAction::TogglePerformanceOverlay` variant (line 15)
- Add field `show_performance_overlay: bool` to `AppMenu` struct (line 59)
- Update `update_debug_states()` to accept overlay state parameter (line 81)
- Add checkbox in Debug menu after Turbo Mode (line 184):
  ```rust
  let mut b = self.show_performance_overlay;
  if ui.checkbox(&mut b, "Performance Overlay").clicked() {
      action = Some(MenuAction::TogglePerformanceOverlay);
      ui.close_menu();
  }
  ```

**Update [main.rs](native-gui/src/main.rs):**
- Add case to `handle_debug_action()` (line 909) to toggle `show_performance_overlay`
- Update `menu.update_debug_states()` call (line 348) to pass overlay state

### 5. Integration Points in Event Loop

**In main event loop ([main.rs](native-gui/src/main.rs)):**

1. **Initialize state** (line 489):
   ```rust
   show_performance_overlay: false,
   perf_tracker: PerformanceTracker::new(),
   ```

2. **Update tracker** (after line 612, after `step_emulator()`):
   ```rust
   app_state.perf_tracker.update(computer.get_cycle_count());
   ```

3. **Render overlay** (line 388, inside `egui_ctx.run()` closure, outside `!exclusive_mode` block):
   ```rust
   if app_state.show_performance_overlay {
       render_performance_overlay(ctx, app_state.turbo_mode, app_state.perf_tracker.get_mhz());
   }
   ```

4. **Update handle_debug_action signature** (line 867): Add `show_performance_overlay: &mut bool` parameter

5. **Pass overlay state to handler** (line 370): Add `&mut app_state.show_performance_overlay`

## Part 2: WASM Implementation

### 1. Performance Tracking in WASM

Add performance tracking fields to `Emu86Computer` struct in [wasm/src/lib.rs](wasm/src/lib.rs) (around line 86):

```rust
#[wasm_bindgen]
pub struct Emu86Computer {
    computer: Computer<WebVideo>,
    // ... existing fields ...

    // NEW: Performance tracking
    perf_last_update_time: f64,      // JS timestamp from performance.now()
    perf_last_cycle_count: u64,      // Cycles at last update
    perf_current_mhz: f64,           // Smoothed actual MHz
    perf_update_interval_ms: f64,    // 200ms update interval
}
```

**Design rationale:** WASM uses JavaScript's `performance.now()` for high-resolution timestamps. Tracking is similar to native but uses f64 timestamps instead of Instant.

### 2. Performance Calculation Methods

Add private helper method to `impl Emu86Computer`:

```rust
impl Emu86Computer {
    fn update_performance(&mut self, current_time_ms: f64) {
        if current_time_ms - self.perf_last_update_time >= self.perf_update_interval_ms {
            let current_cycles = self.computer.get_cycle_count();
            let cycle_delta = current_cycles - self.perf_last_cycle_count;
            let time_delta_ms = current_time_ms - self.perf_last_update_time;

            // Calculate instantaneous MHz: cycles / milliseconds / 1000
            let instant_mhz = (cycle_delta as f64) / time_delta_ms / 1000.0;

            // Exponential moving average for smoothing
            if self.perf_current_mhz == 0.0 {
                self.perf_current_mhz = instant_mhz;
            } else {
                self.perf_current_mhz = 0.7 * self.perf_current_mhz + 0.3 * instant_mhz;
            }

            self.perf_last_update_time = current_time_ms;
            self.perf_last_cycle_count = current_cycles;
        }
    }
}
```

### 3. Export Performance Getter Methods

Add `#[wasm_bindgen]` methods to expose metrics to JavaScript (around line 280):

```rust
#[wasm_bindgen]
impl Emu86Computer {
    /// Get the target clock rate in MHz (always 4.77 for 8086)
    pub fn get_target_mhz(&self) -> f64 {
        4.77
    }

    /// Get the actual measured clock rate in MHz
    pub fn get_actual_mhz(&self) -> f64 {
        self.perf_current_mhz
    }
}
```

**Design rationale:** Separate getter methods are cleaner than returning a tuple/object. JavaScript can call these independently and only when needed.

### 4. Update run_for_ms() to Track Performance

Modify `run_for_ms()` method (line 269) to accept and use current timestamp:

```rust
#[wasm_bindgen]
pub fn run_for_ms(&mut self, ms: f64, current_time_ms: f64) -> bool {
    // Update performance metrics
    self.update_performance(current_time_ms);

    // ... rest of existing logic ...
}
```

### 5. HTML Structure Updates

Add performance display div next to running indicator in [wasm/www/index.html](wasm/www/index.html) (after line 284):

```html
<div id="running-indicator" class="indicator">
    <div class="indicator-led"></div>
    <span class="indicator-text">STOPPED</span>
</div>

<!-- NEW: Performance Display -->
<div id="performance-display" class="performance">
    <div class="perf-label">Target:</div>
    <div class="perf-value" id="perf-target">4.77 MHz</div>
    <div class="perf-label">Actual:</div>
    <div class="perf-value" id="perf-actual">0.00 MHz</div>
</div>
```

### 6. CSS Styling for Performance Display

Add CSS styling in the `<style>` section (around line 300):

```css
.performance {
    display: grid;
    grid-template-columns: auto auto;
    gap: 8px 12px;
    align-items: center;
    margin-left: 20px;
    padding: 8px 12px;
    background-color: #1a2a3a;
    border-radius: 4px;
    font-family: 'Courier New', monospace;
    font-size: 13px;
}

.perf-label {
    color: #8899aa;
    text-align: right;
    font-weight: 600;
}

.perf-value {
    color: #4CAF50;
    font-weight: bold;
}
```

**Design rationale:** Grid layout creates a clean two-column display. Monospace font ensures numbers align properly. Green color matches the running indicator for visual consistency.

### 7. JavaScript Updates for Performance Display

Add performance update function in the `<script>` section (around line 375):

```javascript
function updatePerformanceDisplay() {
    if (!computer) return;

    try {
        const targetMhz = computer.get_target_mhz();
        const actualMhz = computer.get_actual_mhz();

        document.getElementById('perf-target').textContent = `${targetMhz.toFixed(2)} MHz`;
        document.getElementById('perf-actual').textContent = `${actualMhz.toFixed(2)} MHz`;
    } catch (e) {
        // Silently fail if computer not ready
    }
}
```

Update the animation loop to pass timestamp and update display (modify existing `animate()` function around line 462):

```javascript
function animate(timestamp) {
    if (!running || !computer) {
        animationFrameId = null;
        return;
    }

    try {
        // Pass current timestamp to run_for_ms for performance tracking
        const stillRunning = computer.run_for_ms(16, performance.now());

        // Update performance display every frame
        updatePerformanceDisplay();

        if (!stillRunning) {
            running = false;
            updateRunningIndicator(false);
            updateStatus('CPU halted');
            animationFrameId = null;
        } else {
            animationFrameId = requestAnimationFrame(animate);
        }
    } catch (error) {
        running = false;
        updateRunningIndicator(false);
        updateStatus('Error: ' + error.message);
        console.error('Execution error:', error);
        animationFrameId = null;
    }
}
```

Also update the `startExecution()` function to initialize display:

```javascript
function startExecution() {
    if (!computer || running) return;
    running = true;
    updateRunningIndicator(true);
    updatePerformanceDisplay();  // Initial update
    updateStatus('Running...');
    animationFrameId = requestAnimationFrame(animate);
}
```

### 8. Initialize Performance Fields in Constructor

Update `Emu86Computer::new()` constructor (around line 98):

```rust
#[wasm_bindgen]
impl Emu86Computer {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<Emu86Computer, JsValue> {
        // ... existing setup ...

        Ok(Emu86Computer {
            computer,
            video,
            canvas,
            context,
            speaker,
            mouse,

            // NEW: Initialize performance tracking
            perf_last_update_time: 0.0,
            perf_last_cycle_count: 0,
            perf_current_mhz: 0.0,
            perf_update_interval_ms: 200.0,
        })
    }
}
```

## Critical Files

### Native-GUI
1. [native-gui/src/main.rs](native-gui/src/main.rs) - Add PerformanceTracker struct, update AppState, integrate rendering (~80 lines)
2. [native-gui/src/menu.rs](native-gui/src/menu.rs) - Add menu action and checkbox (~15 lines)

### WASM
3. [wasm/src/lib.rs](wasm/src/lib.rs) - Add performance tracking fields and methods to Emu86Computer (~60 lines)
4. [wasm/www/index.html](wasm/www/index.html) - Add HTML structure, CSS styling, JavaScript updates (~80 lines)

## Verification Steps

### Native-GUI Testing
1. **Build and run**: `cargo run -p emu86-native -- --boot --floppy-a examples/hello.img`
2. **Test toggle**: Open Debug menu, click "Performance Overlay" checkbox
3. **Verify throttled mode**: Actual MHz should be near 4.77 MHz, target shows "4.77 MHz"
4. **Verify turbo mode**: Toggle turbo, target should show "Unlimited", actual should be much higher
5. **Test exclusive mode**: Click into emulator window - overlay should remain visible, menu should hide
6. **Test pause**: Toggle pause - actual MHz should drop near zero
7. **Test window resize**: Resize window - overlay should stay anchored to top-right corner
8. **Check performance**: No noticeable lag or stuttering with overlay enabled

### WASM Testing
1. **Build WASM**: `cd wasm && wasm-pack build --target web`
2. **Start dev server**: `cd www && python3 -m http.server 8080` (or use any static server)
3. **Open browser**: Navigate to `http://localhost:8080`
4. **Load disk**: Insert a floppy disk image and boot
5. **Start execution**: Click "Start" button
6. **Verify display**: Performance display should appear next to running indicator
7. **Check target**: Should always show "4.77 MHz"
8. **Check actual**: Should show near 4.77 MHz when running, drop to 0.00 when stopped
9. **Test transitions**: Stop/start multiple times - actual MHz should update smoothly
10. **Check styling**: Performance display should have green text matching running indicator

## Edge Cases

### Native-GUI
- **First few frames**: Display shows 0.00 MHz initially (expected)
- **Paused state**: Actual MHz drops to near-zero (expected behavior)
- **Very high speeds**: Format handles values >1000 MHz correctly with f64

### WASM
- **Initial load**: Performance shows 0.00 MHz until first update interval passes (200ms)
- **Stopped state**: Actual MHz remains at last value when stopped (not updated)
- **Browser tab inactive**: Browser may throttle requestAnimationFrame, affecting displayed MHz
- **Performance.now() precision**: Uses high-resolution timestamps (microsecond precision)

## Future Enhancements

### Native-GUI
- Additional metrics (FPS, instruction count)
- Configurable position (corners, edges)
- Color coding based on performance (green at target, yellow/red when slow)
- Keyboard shortcut toggle (e.g., F11)
- Persistent preferences via config file

### WASM
- FPS counter alongside MHz
- Instructions per second metric
- Visual graph of performance over time (canvas overlay)
- Toggle button to hide/show performance display
- Color-coded actual MHz (green when near target, red when significantly different)
- Browser performance warnings (throttled tab detection)
