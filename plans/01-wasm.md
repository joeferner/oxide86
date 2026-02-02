# WASM Implementation Plan

## Overview
Implement WebAssembly support for emu86 to run the 8086 emulator in web browsers. This includes browser-based keyboard/mouse/video implementations, disk image management via JavaScript, and a web interface for loading and managing floppy and hard drive images.

## Architecture Approach

**Trait-Based Platform Independence**: Follow existing patterns (KeyboardInput, VideoController, MouseInput, DiskBackend)
**WASM-Specific Implementations**: Create web-native implementations using web-sys and wasm-bindgen
**JavaScript Bridge**: Expose clean JavaScript API for controlling the emulator
**Memory-Backed Disks**: In-browser disk images stored in memory, downloadable for persistence
**Browser Integration**: Canvas rendering, File API for uploads, download links for saving changes

## Key Design Decisions

1. **Disk Storage**: Use `MemoryDiskBackend` (in-memory byte arrays) since browser has no direct file system access
2. **Persistence**: User downloads modified disk images to save changes
3. **No Audio Initially**: Skip PC speaker for initial implementation (can add Web Audio API later per audio.md plan)
4. **Single-Threaded**: Run emulator on main thread with `requestAnimationFrame` for timing
5. **No Boot Sector Boot**: Start with pre-loaded DOS programs; boot sector support as future enhancement

## Implementation Phases

### ~~Phase 1: Memory-Backed Disk Storage~~ ✅ COMPLETED

Memory-backed disk storage has been implemented in `core/src/disk.rs` with the `MemoryDiskBackend` struct and exported from `core/src/lib.rs`. The `BackedDisk` struct now includes a `backend()` accessor method to retrieve the underlying backend for operations like downloading disk images.

### ~~Phase 2: WebKeyboard Implementation~~ ✅ COMPLETED

**File**: `wasm/src/web_keyboard.rs` (CREATED)

Web-based keyboard input has been implemented with comprehensive scan code mapping for all standard keyboard keys. The implementation includes:
- Full support for letter keys (A-Z) with shift detection
- Number row keys (0-9) with shift symbols
- Special character keys with shift variants
- Function keys (F1-F12)
- Arrow keys and navigation keys (Home, End, Page Up/Down, Insert, Delete)
- Numpad keys
- Control key combinations (Ctrl+A through Ctrl+Z)
- Browser default behavior prevention for special keys

Dependencies added to `wasm/Cargo.toml`:
- `wasm-bindgen` for WebAssembly bindings
- `web-sys` with keyboard event features
- `js-sys` for JavaScript interop
- `console_error_panic_hook` for better error messages
- `wasm-logger` for browser console logging


### ~~Phase 3: WebMouse Implementation~~ ✅ COMPLETED

**File**: `wasm/src/web_mouse.rs` (CREATED)

Web-based mouse input has been implemented with comprehensive event handling for mouse position and button states. The implementation includes:
- Canvas-based mouse event listeners (mousemove, mousedown, mouseup)
- Coordinate scaling from canvas pixels to DOS graphics resolution (640x200)
- Button state tracking (left, right, middle)
- Mouse motion accumulation in mickeys (8 mickeys per pixel)
- Support for window/canvas resizing via `update_window_size()`
- Proper closure storage to prevent JavaScript garbage collection

Implement mouse input using canvas mouse events:

```rust
use emu86_core::mouse::{MouseInput, MouseState};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, MouseEvent};

/// Web-based mouse input using browser mouse events.
pub struct WebMouse {
    state: Rc<RefCell<MouseState>>,
    _mousemove_closure: Closure<dyn FnMut(MouseEvent)>,
    _mousedown_closure: Closure<dyn FnMut(MouseEvent)>,
    _mouseup_closure: Closure<dyn FnMut(MouseEvent)>,
}

impl WebMouse {
    /// Create a new WebMouse and attach event listeners to canvas.
    pub fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let state = Rc::new(RefCell::new(MouseState::new()));

        let state_move = state.clone();
        let canvas_width = canvas.width() as f64;
        let canvas_height = canvas.height() as f64;

        let mousemove_closure = Closure::wrap(Box::new(move |event: MouseEvent| {
            // Convert canvas coordinates to 8086 text mode coordinates (80x25)
            let x = ((event.offset_x() as f64 / canvas_width) * 80.0) as i16;
            let y = ((event.offset_y() as f64 / canvas_height) * 25.0) as i16;
            state_move.borrow_mut().update_position(x, y);
        }) as Box<dyn FnMut(MouseEvent)>);

        let state_down = state.clone();
        let mousedown_closure = Closure::wrap(Box::new(move |event: MouseEvent| {
            match event.button() {
                0 => state_down.borrow_mut().update_buttons(true, false, false),
                1 => state_down.borrow_mut().update_buttons(false, false, true), // Middle
                2 => state_down.borrow_mut().update_buttons(false, true, false), // Right
                _ => {}
            }
        }) as Box<dyn FnMut(MouseEvent)>);

        let state_up = state.clone();
        let mouseup_closure = Closure::wrap(Box::new(move |_event: MouseEvent| {
            state_up.borrow_mut().update_buttons(false, false, false);
        }) as Box<dyn FnMut(MouseEvent)>);

        // Attach event listeners
        canvas.add_event_listener_with_callback(
            "mousemove",
            mousemove_closure.as_ref().unchecked_ref(),
        )?;
        canvas.add_event_listener_with_callback(
            "mousedown",
            mousedown_closure.as_ref().unchecked_ref(),
        )?;
        canvas.add_event_listener_with_callback(
            "mouseup",
            mouseup_closure.as_ref().unchecked_ref(),
        )?;

        Ok(Self {
            state,
            _mousemove_closure: mousemove_closure,
            _mousedown_closure: mousedown_closure,
            _mouseup_closure: mouseup_closure,
        })
    }
}

impl MouseInput for WebMouse {
    fn get_state(&self) -> MouseState {
        *self.state.borrow()
    }

    fn get_state_change(&mut self) -> MouseState {
        let state = self.state.borrow().clone();
        self.state.borrow_mut().reset_deltas();
        state
    }
}
```

### Phase 4: WebVideo Implementation

**File**: `wasm/src/web_video.rs` (CREATE)

Implement video rendering using HTML5 Canvas:

```rust
use emu86_core::video::{
    CursorPosition, TextAttribute, TextCell, VideoController,
    TEXT_MODE_COLS, TEXT_MODE_ROWS, colors,
};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

/// Web-based video controller using HTML5 Canvas.
pub struct WebVideo {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    char_width: f64,
    char_height: f64,
    cursor_visible: bool,
}

impl WebVideo {
    /// Create a new WebVideo controller.
    ///
    /// # Arguments
    /// * `canvas` - The HTML canvas element to render to
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        // Set canvas size (80x25 text mode)
        canvas.set_width(640);  // 80 chars * 8 pixels
        canvas.set_height(400); // 25 rows * 16 pixels

        let context = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("Failed to get 2d context"))?
            .dyn_into::<CanvasRenderingContext2d>()?;

        // Use monospace font for text rendering
        context.set_font("16px 'Courier New', monospace");

        Ok(Self {
            canvas,
            context,
            char_width: 8.0,
            char_height: 16.0,
            cursor_visible: true,
        })
    }

    /// Convert VGA color index to CSS color string
    fn vga_to_css_color(color: u8) -> &'static str {
        match color & 0x0F {
            colors::BLACK => "#000000",
            colors::BLUE => "#0000AA",
            colors::GREEN => "#00AA00",
            colors::CYAN => "#00AAAA",
            colors::RED => "#AA0000",
            colors::MAGENTA => "#AA00AA",
            colors::BROWN => "#AA5500",
            colors::LIGHT_GRAY => "#AAAAAA",
            colors::DARK_GRAY => "#555555",
            colors::LIGHT_BLUE => "#5555FF",
            colors::LIGHT_GREEN => "#55FF55",
            colors::LIGHT_CYAN => "#55FFFF",
            colors::LIGHT_RED => "#FF5555",
            colors::LIGHT_MAGENTA => "#FF55FF",
            colors::YELLOW => "#FFFF55",
            colors::WHITE => "#FFFFFF",
            _ => "#000000",
        }
    }

    /// Render a single character cell
    fn render_cell(&self, row: usize, col: usize, cell: &TextCell) {
        let x = col as f64 * self.char_width;
        let y = row as f64 * self.char_height;

        // Draw background
        self.context.set_fill_style(&JsValue::from_str(
            Self::vga_to_css_color(cell.attribute.background),
        ));
        self.context.fill_rect(x, y, self.char_width, self.char_height);

        // Draw character
        if cell.character != b' ' {
            self.context.set_fill_style(&JsValue::from_str(
                Self::vga_to_css_color(cell.attribute.foreground),
            ));
            let ch = if cell.character >= 0x20 && cell.character < 0x7F {
                cell.character as char
            } else {
                '?' // Use '?' for non-ASCII characters (or implement CP437)
            };
            self.context
                .fill_text(
                    &ch.to_string(),
                    x,
                    y + self.char_height - 2.0, // Baseline adjustment
                )
                .ok();
        }
    }

    /// Render the cursor
    fn render_cursor(&self, cursor: &CursorPosition) {
        if !self.cursor_visible {
            return;
        }

        let x = cursor.col as f64 * self.char_width;
        let y = cursor.row as f64 * self.char_height;

        self.context.set_fill_style(&JsValue::from_str("#FFFFFF"));
        self.context.fill_rect(
            x,
            y + self.char_height - 2.0,
            self.char_width,
            2.0,
        );
    }
}

impl VideoController for WebVideo {
    fn update_display(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {
        for row in 0..TEXT_MODE_ROWS {
            for col in 0..TEXT_MODE_COLS {
                let index = row * TEXT_MODE_COLS + col;
                self.render_cell(row, col, &buffer[index]);
            }
        }
    }

    fn update_cursor(&mut self, cursor: &CursorPosition) {
        self.render_cursor(cursor);
    }

    fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    fn set_video_mode(&mut self, _mode: u8) {
        // For now, only support mode 0x03 (80x25 text)
        log::warn!("WASM: Video mode changes not yet implemented");
    }
}
```

### Phase 5: WASM Bindings and JavaScript API

**File**: `wasm/Cargo.toml` (MODIFY)

Add dependencies:
```toml
[package]
name = "emu86-wasm"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
emu86-core = { path = "../core" }
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = [
    "Document",
    "Element",
    "HtmlCanvasElement",
    "CanvasRenderingContext2d",
    "KeyboardEvent",
    "MouseEvent",
    "Window",
    "console",
] }
js-sys = "0.3"
console_error_panic_hook = "0.1"
wasm-logger = "0.2"

[lints]
workspace = true
```

**File**: `wasm/src/lib.rs` (REPLACE)

Create main WASM module with JavaScript API:

```rust
use emu86_core::{
    BackedDisk, Computer, DiskGeometry, MemoryDiskBackend, NullMouse,
};
use wasm_bindgen::prelude::*;
use web_sys::{Document, HtmlCanvasElement, Window};

mod web_keyboard;
mod web_mouse;
mod web_video;

use web_keyboard::WebKeyboard;
use web_mouse::WebMouse;
use web_video::WebVideo;

/// Initialize WASM module (call this first from JavaScript)
#[wasm_bindgen(start)]
pub fn init() {
    // Set panic hook for better error messages in browser console
    console_error_panic_hook::set_once();

    // Initialize logging to browser console
    wasm_logger::init(wasm_logger::Config::default());

    log::info!("emu86 WASM module initialized");
}

/// WASM wrapper for the Computer emulator
#[wasm_bindgen]
pub struct Emu86Computer {
    computer: Computer<WebKeyboard, WebVideo>,
    // Store disk backends to enable downloading modified images
    floppy_a: Option<BackedDisk<MemoryDiskBackend>>,
    floppy_b: Option<BackedDisk<MemoryDiskBackend>>,
    hard_drives: Vec<BackedDisk<MemoryDiskBackend>>,
}

#[wasm_bindgen]
impl Emu86Computer {
    /// Create a new emulator instance.
    ///
    /// # Arguments
    /// * `canvas_id` - The ID of the canvas element to render to
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<Emu86Computer, JsValue> {
        let window: Window = web_sys::window()
            .ok_or_else(|| JsValue::from_str("No window object"))?;
        let document: Document = window.document()
            .ok_or_else(|| JsValue::from_str("No document object"))?;

        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str(&format!("Canvas {} not found", canvas_id)))?
            .dyn_into::<HtmlCanvasElement>()?;

        let keyboard = WebKeyboard::new(&document)?;
        let mouse = Box::new(WebMouse::new(&canvas)?);
        let video = WebVideo::new(canvas)?;

        let computer = Computer::new(keyboard, mouse, video);

        Ok(Self {
            computer,
            floppy_a: None,
            floppy_b: None,
            hard_drives: Vec::new(),
        })
    }

    /// Load a floppy disk image from a byte array.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0 = A:, 1 = B:)
    /// * `data` - Disk image data as Uint8Array from JavaScript
    #[wasm_bindgen]
    pub fn load_floppy(&mut self, drive: u8, data: Vec<u8>) -> Result<(), JsValue> {
        let geometry = DiskGeometry::from_size(data.len())
            .ok_or_else(|| JsValue::from_str("Invalid floppy disk size"))?;

        if !geometry.is_floppy() {
            return Err(JsValue::from_str("Image size is not a valid floppy disk"));
        }

        let backend = MemoryDiskBackend::new(data);
        let disk = BackedDisk::new(backend, geometry);

        match drive {
            0 => {
                self.computer.insert_floppy(0, disk.clone());
                self.floppy_a = Some(disk);
                log::info!("Loaded floppy A: ({} bytes)", geometry.total_size);
            }
            1 => {
                self.computer.insert_floppy(1, disk.clone());
                self.floppy_b = Some(disk);
                log::info!("Loaded floppy B: ({} bytes)", geometry.total_size);
            }
            _ => return Err(JsValue::from_str("Invalid floppy drive number (use 0 or 1)")),
        }

        Ok(())
    }

    /// Eject a floppy disk and return its data.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0 = A:, 1 = B:)
    ///
    /// # Returns
    /// The disk image data as Uint8Array, or null if no disk in drive
    #[wasm_bindgen]
    pub fn eject_floppy(&mut self, drive: u8) -> Result<Option<Vec<u8>>, JsValue> {
        match drive {
            0 => {
                self.computer.eject_floppy(0);
                Ok(self.floppy_a.take().map(|disk| {
                    disk.backend().get_data().to_vec()
                }))
            }
            1 => {
                self.computer.eject_floppy(1);
                Ok(self.floppy_b.take().map(|disk| {
                    disk.backend().get_data().to_vec()
                }))
            }
            _ => Err(JsValue::from_str("Invalid floppy drive number (use 0 or 1)")),
        }
    }

    /// Load a hard drive image from a byte array.
    ///
    /// # Arguments
    /// * `data` - Disk image data as Uint8Array from JavaScript
    #[wasm_bindgen]
    pub fn add_hard_drive(&mut self, data: Vec<u8>) -> Result<(), JsValue> {
        let geometry = DiskGeometry::from_size(data.len())
            .ok_or_else(|| JsValue::from_str("Invalid hard drive size"))?;

        if geometry.is_floppy() {
            return Err(JsValue::from_str("Image size is too small for a hard drive"));
        }

        let backend = MemoryDiskBackend::new(data);
        let disk = BackedDisk::new(backend, geometry);

        let drive_letter = (b'C' + self.hard_drives.len() as u8) as char;
        self.computer.add_hard_drive(disk.clone());
        self.hard_drives.push(disk);
        log::info!("Loaded hard drive {}: ({} bytes)", drive_letter, geometry.total_size);

        Ok(())
    }

    /// Get hard drive data for downloading.
    ///
    /// # Arguments
    /// * `drive_index` - Hard drive index (0 = C:, 1 = D:, etc.)
    #[wasm_bindgen]
    pub fn get_hard_drive_data(&self, drive_index: usize) -> Result<Vec<u8>, JsValue> {
        self.hard_drives
            .get(drive_index)
            .map(|disk| disk.backend().get_data().to_vec())
            .ok_or_else(|| JsValue::from_str("Hard drive index out of range"))
    }

    /// Boot from a drive.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0x00 = A:, 0x01 = B:, 0x80 = C:)
    #[wasm_bindgen]
    pub fn boot(&mut self, drive: u8) -> Result<(), JsValue> {
        self.computer
            .boot(drive)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Execute one instruction and return whether CPU is still running.
    #[wasm_bindgen]
    pub fn step(&mut self) -> Result<bool, JsValue> {
        self.computer
            .step()
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Execute instructions for approximately the given number of milliseconds.
    ///
    /// # Arguments
    /// * `ms` - Milliseconds to run (approximately)
    ///
    /// # Returns
    /// true if CPU is still running, false if halted
    #[wasm_bindgen]
    pub fn run_for_ms(&mut self, ms: f64) -> Result<bool, JsValue> {
        // 8086 at 4.77 MHz: approximately 4770 cycles per ms
        let cycles = (ms * 4770.0) as u64;
        let mut remaining = cycles;

        while remaining > 0 {
            let running = self.step()?;
            if !running {
                return Ok(false);
            }
            // Rough approximation: assume average instruction takes ~10 cycles
            remaining = remaining.saturating_sub(10);
        }

        Ok(true)
    }

    /// Reset the computer.
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.computer.reset();
        log::info!("Computer reset");
    }
}
```

### Phase 6: Web Interface

**File**: `wasm/www/index.html` (CREATE)

Create HTML interface for the emulator:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>emu86 - 8086 Emulator</title>
    <style>
        body {
            font-family: Arial, sans-serif;
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
            background-color: #f0f0f0;
        }
        h1 {
            text-align: center;
        }
        #emulator-container {
            background-color: #000;
            padding: 10px;
            border-radius: 5px;
            text-align: center;
        }
        canvas {
            border: 2px solid #333;
            background-color: #000;
            image-rendering: pixelated;
            image-rendering: crisp-edges;
        }
        .controls {
            margin-top: 20px;
            background-color: #fff;
            padding: 15px;
            border-radius: 5px;
        }
        .control-group {
            margin-bottom: 15px;
        }
        label {
            display: inline-block;
            width: 120px;
            font-weight: bold;
        }
        button {
            padding: 8px 16px;
            margin: 5px;
            cursor: pointer;
        }
        .file-input {
            margin-left: 10px;
        }
        #status {
            margin-top: 10px;
            padding: 10px;
            background-color: #e8f4f8;
            border-radius: 3px;
            font-family: monospace;
        }
    </style>
</head>
<body>
    <h1>emu86 - Intel 8086 Emulator</h1>

    <div id="emulator-container">
        <canvas id="display"></canvas>
    </div>

    <div class="controls">
        <div class="control-group">
            <label>Floppy A:</label>
            <input type="file" id="floppy-a-input" class="file-input" accept=".img,.ima,.dsk">
            <button id="load-floppy-a">Load</button>
            <button id="eject-floppy-a">Eject & Download</button>
        </div>

        <div class="control-group">
            <label>Floppy B:</label>
            <input type="file" id="floppy-b-input" class="file-input" accept=".img,.ima,.dsk">
            <button id="load-floppy-b">Load</button>
            <button id="eject-floppy-b">Eject & Download</button>
        </div>

        <div class="control-group">
            <label>Hard Drive C:</label>
            <input type="file" id="hdd-input" class="file-input" accept=".img,.ima,.dsk,.vhd">
            <button id="load-hdd">Load</button>
            <button id="download-hdd">Download</button>
        </div>

        <div class="control-group">
            <label>Boot:</label>
            <button id="boot-a">Boot from A:</button>
            <button id="boot-c">Boot from C:</button>
            <button id="reset">Reset</button>
        </div>

        <div class="control-group">
            <label>Execution:</label>
            <button id="start">Start</button>
            <button id="stop">Stop</button>
            <button id="step">Step</button>
        </div>

        <div id="status">Ready. Load a disk image and click Boot.</div>
    </div>

    <script type="module">
        import init, { Emu86Computer } from './pkg/emu86_wasm.js';

        let computer = null;
        let running = false;
        let animationFrameId = null;

        async function main() {
            await init();
            computer = new Emu86Computer('display');
            updateStatus('Emulator initialized. Load disk images to begin.');
        }

        function updateStatus(message) {
            document.getElementById('status').textContent = message;
        }

        // File loading helpers
        async function loadFile(file) {
            return new Promise((resolve, reject) => {
                const reader = new FileReader();
                reader.onload = (e) => resolve(new Uint8Array(e.target.result));
                reader.onerror = reject;
                reader.readAsArrayBuffer(file);
            });
        }

        function downloadFile(data, filename) {
            const blob = new Blob([data], { type: 'application/octet-stream' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = filename;
            a.click();
            URL.revokeObjectURL(url);
        }

        // Event handlers
        document.getElementById('load-floppy-a').addEventListener('click', async () => {
            const input = document.getElementById('floppy-a-input');
            if (input.files.length === 0) return;

            try {
                const data = await loadFile(input.files[0]);
                computer.load_floppy(0, data);
                updateStatus(`Loaded floppy A: ${input.files[0].name} (${data.length} bytes)`);
            } catch (e) {
                updateStatus(`Error loading floppy A: ${e}`);
            }
        });

        document.getElementById('eject-floppy-a').addEventListener('click', () => {
            try {
                const data = computer.eject_floppy(0);
                if (data) {
                    downloadFile(data, 'floppy-a.img');
                    updateStatus('Floppy A ejected and downloaded');
                } else {
                    updateStatus('No disk in floppy A');
                }
            } catch (e) {
                updateStatus(`Error ejecting floppy A: ${e}`);
            }
        });

        // Similar handlers for floppy B and HDD...

        document.getElementById('boot-a').addEventListener('click', () => {
            try {
                computer.boot(0x00);
                updateStatus('Booted from floppy A:');
            } catch (e) {
                updateStatus(`Boot error: ${e}`);
            }
        });

        document.getElementById('boot-c').addEventListener('click', () => {
            try {
                computer.boot(0x80);
                updateStatus('Booted from hard drive C:');
            } catch (e) {
                updateStatus(`Boot error: ${e}`);
            }
        });

        document.getElementById('start').addEventListener('click', () => {
            if (running) return;
            running = true;
            updateStatus('Running...');

            function frame(timestamp) {
                if (!running) return;

                try {
                    // Run for ~16ms per frame (60 FPS)
                    const stillRunning = computer.run_for_ms(16);
                    if (!stillRunning) {
                        running = false;
                        updateStatus('CPU halted');
                        return;
                    }
                    animationFrameId = requestAnimationFrame(frame);
                } catch (e) {
                    running = false;
                    updateStatus(`Error: ${e}`);
                }
            }

            animationFrameId = requestAnimationFrame(frame);
        });

        document.getElementById('stop').addEventListener('click', () => {
            running = false;
            if (animationFrameId) {
                cancelAnimationFrame(animationFrameId);
            }
            updateStatus('Stopped');
        });

        document.getElementById('step').addEventListener('click', () => {
            try {
                const stillRunning = computer.step();
                updateStatus(stillRunning ? 'Stepped 1 instruction' : 'CPU halted');
            } catch (e) {
                updateStatus(`Error: ${e}`);
            }
        });

        document.getElementById('reset').addEventListener('click', () => {
            computer.reset();
            running = false;
            updateStatus('Reset');
        });

        main();
    </script>
</body>
</html>
```

### Phase 7: Build Configuration

**File**: `wasm/.cargo/config.toml` (CREATE)

```toml
[build]
target = "wasm32-unknown-unknown"
```

**File**: `wasm/build.sh` (CREATE)

Build script for WASM:

```bash
#!/bin/bash
set -e

# Build WASM with wasm-pack
wasm-pack build --target web --out-dir www/pkg

# Copy index.html if needed
cp index.html www/ 2>/dev/null || true

echo "Build complete. Serve www/ directory with a local web server."
echo "Example: python3 -m http.server --directory www 8080"
```

### Phase 8: Development Server Setup

**File**: `wasm/README.md` (CREATE)

```markdown
# emu86 WASM

WebAssembly build of the emu86 8086 emulator.

## Building

Install wasm-pack:
```bash
cargo install wasm-pack
```

Build the WASM module:
```bash
./build.sh
```

## Running

Serve the www/ directory with any HTTP server:

```bash
# Python
python3 -m http.server --directory www 8080

# Or use another server
cd www && npx serve
```

Open http://localhost:8080 in your browser.

## Usage

1. Click "Load" next to Floppy A: and select a .img file
2. Click "Boot from A:" to boot the disk
3. Click "Start" to begin execution
4. Use "Eject & Download" to save any changes made to the disk

## Supported Disk Images

- Floppy disks: 1.44MB, 720KB, 360KB
- Hard drives: Any size >= 2MB (sector-aligned)

## File Format

All disk images should be raw sector dumps (no headers).
```

## Critical Files Summary

| File | Action | Purpose |
|------|--------|---------|
| `core/src/disk.rs` | MODIFY | Add MemoryDiskBackend for in-memory disk storage |
| `core/src/lib.rs` | MODIFY | Export MemoryDiskBackend |
| `wasm/src/web_keyboard.rs` | CREATE | Browser keyboard input via JavaScript events |
| `wasm/src/web_mouse.rs` | CREATE | Browser mouse input via canvas events |
| `wasm/src/web_video.rs` | CREATE | Canvas-based video rendering |
| `wasm/src/lib.rs` | REPLACE | Main WASM module with JavaScript API |
| `wasm/Cargo.toml` | MODIFY | Add wasm-bindgen dependencies |
| `wasm/www/index.html` | CREATE | Web interface for emulator |
| `wasm/build.sh` | CREATE | Build script using wasm-pack |
| `wasm/.cargo/config.toml` | CREATE | Set default WASM target |
| `wasm/README.md` | CREATE | Documentation for WASM build |

## JavaScript API

The `Emu86Computer` class exposed to JavaScript provides:

### Constructor
- `new Emu86Computer(canvasId)` - Create emulator instance

### Disk Management
- `load_floppy(drive, data)` - Load floppy (drive: 0=A:, 1=B:)
- `eject_floppy(drive)` - Eject and get disk data
- `add_hard_drive(data)` - Add hard drive (C:, D:, etc.)
- `get_hard_drive_data(index)` - Get hard drive data for download

### Execution Control
- `boot(drive)` - Boot from drive (0x00=A:, 0x80=C:)
- `step()` - Execute one instruction
- `run_for_ms(milliseconds)` - Execute for time period
- `reset()` - Reset computer

## User Workflow

### Loading Floppy Disks

1. User clicks "Choose File" button
2. Browser File API opens file picker
3. User selects .img file
4. JavaScript reads file as ArrayBuffer
5. Convert to Uint8Array and pass to `load_floppy()`
6. WASM creates MemoryDiskBackend with data
7. Disk ready for use

### Saving Modified Disks

1. User clicks "Eject & Download"
2. JavaScript calls `eject_floppy()`
3. WASM returns disk data as Uint8Array
4. JavaScript creates Blob from data
5. Trigger browser download with URL.createObjectURL()
6. User saves .img file to local filesystem

### Hard Drive Workflow

1. Load: Same as floppy (File API → `add_hard_drive()`)
2. Save: Click "Download" → `get_hard_drive_data()` → browser download
3. Multiple hard drives: Load multiple files, they become C:, D:, E:, etc.

## Verification Strategy

### Test 1: Basic Execution
Create simple .COM file that writes "HELLO" to screen:
```nasm
org 0x100
mov ah, 0x02
mov dl, 'H'
int 0x21
mov dl, 'E'
int 0x21
; ... etc
int 0x20
```
Load to floppy, boot, verify output on canvas.

### Test 2: Keyboard Input
Program that waits for key and echoes it:
```nasm
mov ah, 0x01  ; Read char with echo
int 0x21
int 0x20
```
Type on keyboard, verify character appears.

### Test 3: Mouse Input
Use INT 33h mouse functions to show cursor and get position.

### Test 4: Disk Persistence
1. Boot DOS
2. Create file with EDIT or COPY CON
3. Eject and download disk
4. Reload disk
5. Verify file still exists

### Test 5: Multi-Drive
1. Load different disks in A: and B:
2. Boot from A:
3. Use `COPY A:FILE.TXT B:` to copy between drives
4. Eject B: and verify file copied

## Implementation Notes

### Canvas Rendering Performance
- Full screen redraw every frame is acceptable for text mode (80x25 = 2000 cells)
- Could optimize by tracking dirty cells if needed
- Use `requestAnimationFrame` for smooth rendering

### Keyboard Event Handling
- Attach to document to capture all keys
- `preventDefault()` on special keys (arrows, F keys) to avoid browser navigation
- Build scan code mapping table based on 8086 keyboard scan codes

### Memory Management
- Disk images stored in WASM linear memory (Vec<u8>)
- For large hard drives (e.g., 100MB), consider memory limits
- Typical browser WASM memory limit: 4GB, plenty for disk images

### Security Considerations
- All code runs in browser sandbox
- No access to user's file system except via File API
- Disk modifications only affect in-memory copy until user downloads

### Browser Compatibility
Target modern browsers with WASM support:
- Chrome 57+
- Firefox 52+
- Safari 11+
- Edge 16+

### Performance Expectations
- JavaScript/WASM overhead adds ~10-20% vs native
- Sufficient for running DOS programs smoothly
- May need throttling for "turbo" mode

## Future Enhancements

### IndexedDB Persistence
Store disk images in browser's IndexedDB for automatic saving:
```javascript
// Save disk to IndexedDB when modified
function saveDisk(name, data) {
    const db = await openDB('emu86-disks', 1);
    await db.put('disks', { name, data });
}

// Load disk from IndexedDB on startup
async function loadDisk(name) {
    const db = await openDB('emu86-disks', 1);
    return await db.get('disks', name);
}
```

### Web Audio API for PC Speaker
Implement per [plans/01-audio.md](plans/01-audio.md) Phase 7:
- Create `WebSpeaker` with OscillatorNode
- Set type to "square" for square wave
- Control frequency via `oscillator.frequency.setValueAtTime()`
- Handle user interaction requirement for AudioContext

### Gamepad API for Joystick
Map browser Gamepad API to 8086 joystick port (0x201).

### Share Disk Images via URL
Generate shareable links that encode disk image in URL or cloud storage.

### CP437 Font Rendering
Use proper IBM PC font bitmap for authentic character rendering instead of Unicode equivalents.

### CGA Graphics Mode (Implemented in plans/03-cga.md)

Canvas-based pixel rendering for CGA graphics modes:

**320x200, 4-color mode**:
- Resize canvas to 640x400 (2x scaling)
- Use ImageData API for direct pixel buffer manipulation
- Map 2-bit pixel values to CGA palette colors
- Render scaled via `putImageData()` + `scale()`

**640x200, 2-color mode**:
- Resize canvas to 1280x400 (2x scaling)
- 1-bit pixels mapped to foreground/background colors
- Same ImageData approach for performance

**Palette Changes**:
- Update on I/O port 0x3D9 writes
- Re-render entire frame with new palette colors
- No additional ImageData allocation needed

See [plans/03-cga.md](03-cga.md) for full CGA implementation details.

## Testing Commands

```bash
# Build WASM
cd wasm
./build.sh

# Serve locally
python3 -m http.server --directory www 8080

# Open in browser
open http://localhost:8080
```

## Success Criteria

- [ ] MemoryDiskBackend compiles and works in core
- [ ] WASM builds without errors using wasm-pack
- [ ] Web page loads and displays canvas
- [ ] Keyboard events are captured and buffered
- [ ] Mouse events work on canvas
- [ ] Can load floppy disk via File API
- [ ] Boot from floppy succeeds
- [ ] Video renders to canvas correctly
- [ ] Can execute DOS programs that use keyboard/video
- [ ] Eject and download returns modified disk image
- [ ] Hard drive loading and access works
- [ ] Multiple drives (A:, B:, C:) work simultaneously
