# Oxide86 WASM

WebAssembly build of the oxide86 x86 emulator for running in web browsers.

## Prerequisites

Install [wasm-pack](https://rustwasm.github.io/wasm-pack/):

```bash
cargo install wasm-pack
```

## Building

Build the WASM module:

```bash
./scripts/build.sh
```

This will compile the emulator to WebAssembly and generate JavaScript bindings in `www/pkg/`.

## Running

Serve the www/ directory with any HTTP server:

```bash
# Using Python (recommended)
cd www
python3 -m http.server 8080
```

Then open http://localhost:8080 in your browser.

## Usage

1. **Load Disk Images**
   - Click "Choose File" next to Floppy A:, B:, or Hard Drive C:
   - Select a disk image file (.img, .ima, .dsk)
   - Click the corresponding "Load" button

2. **Boot the Emulator**
   - Click "Boot from A:" to boot from floppy A:
   - Click "Boot from C:" to boot from hard drive C:

3. **Control Execution**
   - Click "Start" to begin running the emulator at ~60 FPS
   - Click "Stop" to pause execution
   - Click "Step" to execute a single instruction
   - Click "Reset" to restart the computer

4. **Interact with the Emulator**
   - Click inside the canvas to focus
   - Type on your keyboard to send input to the emulator
   - Mouse input is automatically captured when over the canvas

## Supported Disk Images

### Floppy Disks
- 1.44 MB (2880 sectors)
- 720 KB (1440 sectors)
- 360 KB (720 sectors)

### Hard Drives
- Any size >= 2 MB (must be sector-aligned, 512 bytes/sector)
- Supports MBR partitions

All disk images should be raw sector dumps (no headers or compression).

## Architecture

The WASM implementation provides browser-based versions of the platform-independent traits:

- `WebKeyboard` - Captures browser keyboard events and converts to 8086 scan codes
- `WebMouse` - Handles canvas mouse events and tracks position/buttons
- `WebVideo` - Renders to HTML5 Canvas using authentic CP437 font and VGA palette
- `MemoryDiskBackend` - Stores disk images in memory (no browser filesystem access)

## JavaScript API

The `Oxide86Computer` class exposed to JavaScript provides:

### Constructor
```javascript
const computer = new Oxide86Computer('canvas-id');
```

### Disk Management
```javascript
computer.load_floppy(drive, data);  // drive: 0=A:, 1=B:; data: Uint8Array
computer.eject_floppy(drive);       // drive: 0=A:, 1=B:
computer.set_hard_drive(drive, data); // drive: 0x80=C:, 0x81=D:; data: Uint8Array
```

### Execution Control
```javascript
computer.boot(drive);        // drive: 0x00=A:, 0x80=C:
computer.step();             // Returns: bool (true if running)
computer.run_for_ms(ms);     // Returns: bool (true if running)
computer.reset();            // Reset computer state
```

## Performance

The emulator runs at approximately 4.77 MHz (authentic 8086 speed) when using `run_for_ms()`. The web interface targets 60 FPS with ~16ms of execution per frame, providing smooth DOS program execution.

## Browser Compatibility

Requires modern browsers with WebAssembly support:
- Chrome 57+
- Firefox 52+
- Safari 11+
- Edge 16+

## Limitations

- No disk persistence between sessions (load/eject only)
- No audio output (PC speaker not implemented)
- Text mode only (80x25 characters at 640x400 pixels)
- All disk changes are in-memory only

## Troubleshooting

**Emulator won't start:**
- Ensure you've loaded a valid disk image
- Check browser console for error messages
- Verify the disk image has a valid boot sector (0x55AA signature)

**Keyboard not working:**
- Click inside the canvas area to focus
- Check browser console for warnings about prevented defaults

**Display issues:**
- Ensure canvas is rendering (black rectangle should be visible)
- Try resizing the browser window
- Check for WebGL/Canvas API support in your browser

## Development

### Project Structure
```
wasm/
├── src/
│   ├── lib.rs           # Main WASM module and JavaScript API
│   ├── web_keyboard.rs  # Browser keyboard input
│   ├── web_mouse.rs     # Canvas mouse input
│   └── web_video.rs     # Canvas rendering
├── www/
│   ├── index.html       # Web interface
│   └── pkg/             # Generated WASM bindings (after build)
├── build.sh             # Build script
└── README.md            # This file
```

### Building for Development
```bash
# Build with debug info
wasm-pack build --dev --target web --out-dir www/pkg

# Build optimized for production
wasm-pack build --release --target web --out-dir www/pkg
```

### Debugging
Open browser developer tools to view:
- Console logs from `log::info!`, `log::warn!`, etc.
- WASM execution errors and panics
- Network requests for disk images
