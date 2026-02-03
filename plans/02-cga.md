# CGA Graphics Support Implementation Plan

## Status
- ✅ **Phase 1: Core Graphics State Management** - COMPLETED
- ✅ **Phase 2: VideoController Trait Extension** - COMPLETED

## Overview
Implement IBM Color Graphics Adapter (CGA) graphics modes alongside existing text mode support. This enables running graphics-based DOS programs and games from the early 1980s.

## Architecture Approach

**Dual-Mode Video**: Extend existing `Video` struct to support both text and graphics modes
**Trait-Based Rendering**: Extend `VideoController` trait with graphics rendering methods
**CGA-Accurate Memory Layout**: Implement authentic interlaced memory mapping
**Palette Support**: Implement CGA color palettes and I/O port control
**Mode Switching**: Seamless transitions between text and graphics modes
**WASM Compatible**: Architecture supports both native and WASM rendering

## CGA Graphics Modes

| Mode | Resolution | Colors | Memory Layout | Notes |
|------|------------|--------|---------------|-------|
| 0x04 | 320x200    | 4      | Interlaced    | Most common CGA graphics mode |
| 0x05 | 320x200    | 4      | Interlaced    | Same as 0x04 but monochrome palette |
| 0x06 | 640x200    | 2      | Interlaced    | High-resolution monochrome |

**Text Modes (Already Supported)**:
- 0x00, 0x01: 40x25 text
- 0x02, 0x03: 80x25 text
- 0x07: 80x25 monochrome text

## CGA Memory Layout

### Text Mode (Current)
- Linear layout: `[char, attr, char, attr, ...]`
- Size: 80×25×2 = 4000 bytes
- Base address: 0xB8000

### Graphics Mode (320x200, 4-color)
- **Interlaced layout** (critical for CGA compatibility):
  - Even scan lines (0, 2, 4, ..., 198): 0xB8000 - 0xB9F3F (8000 bytes)
  - Odd scan lines (1, 3, 5, ..., 199): 0xBA000 - 0xBBF3F (8000 bytes)
- **Pixel packing**: 4 pixels per byte (2 bits per pixel)
  - Byte format: `[px0:2][px1:2][px2:2][px3:2]` (MSB first)
- **Total size**: 16000 bytes

### Graphics Mode (640x200, 2-color)
- Same interlaced layout
- **Pixel packing**: 8 pixels per byte (1 bit per pixel)
- **Total size**: 16000 bytes

## CGA Color Palettes

### Palette 0 (Background + 3 colors)
- Background: Programmable (16 colors)
- Color 1: Green (or Cyan in high-intensity mode)
- Color 2: Red (or Magenta in high-intensity mode)
- Color 3: Brown (or White in high-intensity mode)

### Palette 1 (Background + 3 colors)
- Background: Programmable (16 colors)
- Color 1: Cyan (or Light Cyan in high-intensity mode)
- Color 2: Magenta (or Light Magenta in high-intensity mode)
- Color 3: White (or Bright White in high-intensity mode)

### Palette Selection
- Controlled via I/O port 0x3D9 (Color Select Register)
- Bit 0-3: Background color (16 colors)
- Bit 4: Intensity (bright colors vs normal)
- Bit 5: Palette select (0 = palette 0, 1 = palette 1)

## Implementation Phases

### Phase 1: Core Graphics State Management ✅ COMPLETED

**File**: `core/src/video.rs` (MODIFIED)

**What was implemented:**
- ✅ Added `VideoMode` enum (Text, Graphics320x200, Graphics640x200)
- ✅ Added `GraphicsBuffer` struct with interlaced CGA memory layout support
- ✅ Added `CgaPalette` struct with CGA color palette management
- ✅ Extended `Video` struct with graphics mode fields (graphics_buffer, mode_type, palette)
- ✅ Updated `Video::set_mode()` to allocate/deallocate graphics buffers
- ✅ Updated `Video::read_byte()` and `write_byte()` to handle both text and graphics modes
- ✅ Added helper methods: `is_graphics_mode()`, `get_mode_type()`, `set_palette()`, `get_palette()`, `get_graphics_buffer()`
- ✅ Extended `VideoController` trait with `update_graphics_320x200()` and `update_graphics_640x200()` methods

**Implementation details:**

```rust
/// Video mode type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoMode {
    /// Text modes: 80x25 or 40x25
    Text { cols: usize, rows: usize },
    /// CGA 320x200, 4 colors
    Graphics320x200,
    /// CGA 640x200, 2 colors
    Graphics640x200,
}

/// Graphics framebuffer for CGA modes
pub struct GraphicsBuffer {
    /// Raw pixel data (16KB for CGA modes)
    /// Interlaced: first 8KB = even scan lines, second 8KB = odd scan lines
    data: Vec<u8>,
    /// Width in pixels
    width: usize,
    /// Height in pixels
    height: usize,
    /// Bits per pixel (1 for 640x200, 2 for 320x200)
    bits_per_pixel: u8,
}

impl GraphicsBuffer {
    pub fn new_320x200() -> Self {
        Self {
            data: vec![0; 16000], // 320x200 / 4 pixels per byte
            width: 320,
            height: 200,
            bits_per_pixel: 2,
        }
    }

    pub fn new_640x200() -> Self {
        Self {
            data: vec![0; 16000], // 640x200 / 8 pixels per byte
            width: 640,
            height: 200,
            bits_per_pixel: 1,
        }
    }

    /// Convert linear framebuffer offset to interlaced CGA memory offset
    /// CGA uses interlaced memory: even lines at 0x0000-0x1F3F, odd at 0x2000-0x3F3F
    fn linear_to_interlaced(&self, offset: usize) -> usize {
        let bytes_per_line = self.width * (self.bits_per_pixel as usize) / 8;
        let line = offset / bytes_per_line;
        let col = offset % bytes_per_line;

        if line % 2 == 0 {
            // Even line: bank 0 (0x0000-0x1F3F)
            (line / 2) * bytes_per_line + col
        } else {
            // Odd line: bank 1 (0x2000-0x3F3F), offset by 8KB
            0x2000 + (line / 2) * bytes_per_line + col
        }
    }

    /// Convert interlaced CGA memory offset to linear framebuffer offset
    fn interlaced_to_linear(&self, offset: usize) -> usize {
        let bytes_per_line = self.width * (self.bits_per_pixel as usize) / 8;

        if offset < 0x2000 {
            // Even line bank
            let line_in_bank = offset / bytes_per_line;
            let col = offset % bytes_per_line;
            (line_in_bank * 2) * bytes_per_line + col
        } else {
            // Odd line bank
            let offset_in_bank = offset - 0x2000;
            let line_in_bank = offset_in_bank / bytes_per_line;
            let col = offset_in_bank % bytes_per_line;
            (line_in_bank * 2 + 1) * bytes_per_line + col
        }
    }

    /// Read byte from graphics memory (using interlaced addressing)
    pub fn read_byte(&self, offset: usize) -> u8 {
        if offset >= self.data.len() {
            return 0;
        }
        let linear_offset = self.interlaced_to_linear(offset);
        self.data[linear_offset]
    }

    /// Write byte to graphics memory (using interlaced addressing)
    pub fn write_byte(&mut self, offset: usize, value: u8) {
        if offset >= self.data.len() {
            return;
        }
        let linear_offset = self.interlaced_to_linear(offset);
        self.data[linear_offset] = value;
    }

    /// Get pixel data as linear buffer (for rendering)
    pub fn get_pixels(&self) -> &[u8] {
        &self.data
    }
}

/// CGA color palette state
#[derive(Debug, Clone, Copy)]
pub struct CgaPalette {
    /// Background color (4 bits, 16 colors)
    pub background: u8,
    /// Palette select (0 or 1)
    pub palette_id: u8,
    /// Intensity/bright mode enabled
    pub intensity: bool,
}

impl CgaPalette {
    pub fn new() -> Self {
        Self {
            background: 0,
            palette_id: 0,
            intensity: false,
        }
    }

    /// Get the 4 colors for current palette
    /// Returns [background, color1, color2, color3]
    pub fn get_colors(&self) -> [u8; 4] {
        let bg = self.background;

        if self.palette_id == 0 {
            // Palette 0: Green, Red, Brown (or Cyan, Magenta, White with intensity)
            if self.intensity {
                [bg, colors::CYAN, colors::LIGHT_RED, colors::WHITE]
            } else {
                [bg, colors::GREEN, colors::RED, colors::BROWN]
            }
        } else {
            // Palette 1: Cyan, Magenta, White (or Light variants with intensity)
            if self.intensity {
                [bg, colors::LIGHT_CYAN, colors::LIGHT_MAGENTA, colors::WHITE]
            } else {
                [bg, colors::CYAN, colors::MAGENTA, colors::LIGHT_GRAY]
            }
        }
    }

    /// Parse from CGA Color Select Register (port 0x3D9)
    pub fn from_register(value: u8) -> Self {
        Self {
            background: value & 0x0F,
            palette_id: (value >> 5) & 0x01,
            intensity: (value & 0x10) != 0,
        }
    }

    /// Convert to Color Select Register value
    pub fn to_register(&self) -> u8 {
        let mut value = self.background & 0x0F;
        if self.intensity {
            value |= 0x10;
        }
        value |= (self.palette_id & 0x01) << 5;
        value
    }
}

impl Default for CgaPalette {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Video {
    /// Current cursor position (text mode only)
    cursor: CursorPosition,
    /// Text mode buffer
    text_buffer: [TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS],
    /// Graphics mode buffer (optional, allocated when in graphics mode)
    graphics_buffer: Option<GraphicsBuffer>,
    /// Current video mode
    mode: u8,
    /// Parsed video mode type
    mode_type: VideoMode,
    /// Active display page (text mode only)
    active_page: u8,
    /// CGA palette state (graphics mode only)
    palette: CgaPalette,
    /// Dirty flag
    dirty: bool,
}

impl Video {
    pub fn new() -> Self {
        Self {
            cursor: CursorPosition::default(),
            text_buffer: [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS],
            graphics_buffer: None,
            mode: 0x03,
            mode_type: VideoMode::Text { cols: TEXT_MODE_COLS, rows: TEXT_MODE_ROWS },
            active_page: 0,
            palette: CgaPalette::new(),
            dirty: false,
        }
    }

    /// Set video mode (called from INT 10h AH=00h)
    pub fn set_mode(&mut self, mode: u8) {
        self.mode = mode;

        // Determine mode type and allocate appropriate buffer
        self.mode_type = match mode {
            0x00 | 0x01 => VideoMode::Text { cols: 40, rows: 25 },
            0x02 | 0x03 | 0x07 => VideoMode::Text { cols: 80, rows: 25 },
            0x04 | 0x05 => {
                self.graphics_buffer = Some(GraphicsBuffer::new_320x200());
                VideoMode::Graphics320x200
            }
            0x06 => {
                self.graphics_buffer = Some(GraphicsBuffer::new_640x200());
                VideoMode::Graphics640x200
            }
            _ => {
                log::warn!("Unsupported video mode 0x{:02X}, defaulting to text", mode);
                VideoMode::Text { cols: 80, rows: 25 }
            }
        };

        // Clear buffers on mode change
        if matches!(self.mode_type, VideoMode::Text { .. }) {
            self.text_buffer = [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS];
            self.graphics_buffer = None;
        }

        self.dirty = true;
        log::info!("Video mode set to 0x{:02X} ({:?})", mode, self.mode_type);
    }

    /// Check if currently in graphics mode
    pub fn is_graphics_mode(&self) -> bool {
        !matches!(self.mode_type, VideoMode::Text { .. })
    }

    /// Get current mode type
    pub fn get_mode_type(&self) -> VideoMode {
        self.mode_type
    }

    /// Set CGA palette (from I/O port 0x3D9)
    pub fn set_palette(&mut self, value: u8) {
        self.palette = CgaPalette::from_register(value);
        self.dirty = true;
    }

    /// Get CGA palette register value
    pub fn get_palette(&self) -> u8 {
        self.palette.to_register()
    }

    /// Read byte from video memory (handles both text and graphics)
    pub fn read_byte(&self, offset: usize) -> u8 {
        match &self.mode_type {
            VideoMode::Text { .. } => {
                // Text mode: existing logic
                if offset >= TEXT_MODE_BUFFER_SIZE {
                    return 0;
                }
                let cell_index = offset / 2;
                if cell_index >= self.text_buffer.len() {
                    return 0;
                }
                if offset.is_multiple_of(2) {
                    self.text_buffer[cell_index].character
                } else {
                    self.text_buffer[cell_index].attribute.to_byte()
                }
            }
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200 => {
                // Graphics mode
                if let Some(ref buffer) = self.graphics_buffer {
                    buffer.read_byte(offset)
                } else {
                    0
                }
            }
        }
    }

    /// Write byte to video memory (handles both text and graphics)
    pub fn write_byte(&mut self, offset: usize, value: u8) {
        match &self.mode_type {
            VideoMode::Text { .. } => {
                // Text mode: existing logic
                if offset >= TEXT_MODE_BUFFER_SIZE {
                    return;
                }
                let cell_index = offset / 2;
                if cell_index >= self.text_buffer.len() {
                    return;
                }
                if offset.is_multiple_of(2) {
                    self.text_buffer[cell_index].character = value;
                } else {
                    self.text_buffer[cell_index].attribute = TextAttribute::from_byte(value);
                }
            }
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200 => {
                // Graphics mode
                if let Some(ref mut buffer) = self.graphics_buffer {
                    buffer.write_byte(offset, value);
                }
            }
        }
        self.dirty = true;
    }

    /// Get graphics buffer (for rendering)
    pub fn get_graphics_buffer(&self) -> Option<&GraphicsBuffer> {
        self.graphics_buffer.as_ref()
    }

    /// Get palette (for rendering)
    pub fn get_palette(&self) -> &CgaPalette {
        &self.palette
    }

    // Keep existing text mode methods...
}
```

### Phase 2: VideoController Trait Extension ✅ COMPLETED

**File**: `core/src/video.rs` (MODIFIED)

**What was implemented:**
- ✅ Added `update_graphics_320x200()` method to VideoController trait (lines 281-287)
- ✅ Added `update_graphics_640x200()` method to VideoController trait (lines 290-294)
- ✅ Both methods have default implementations that log warnings
- ✅ Platform implementations (TerminalVideo, WebVideo) use default implementations
- ✅ WASM compatibility verified - trait is safe for both native and WASM targets

### Phase 3: CGA I/O Ports

**File**: `core/src/io/cga_ports.rs` (CREATE)

Implement CGA-specific I/O ports:

```rust
/// CGA Mode Control Register (port 0x3D8)
#[derive(Debug, Clone, Copy)]
pub struct CgaModeControl {
    value: u8,
}

impl CgaModeControl {
    pub fn new() -> Self {
        Self { value: 0x00 }
    }

    /// Bit 0: 80x25 text mode (0 = 40x25, 1 = 80x25)
    pub fn is_80_column(&self) -> bool {
        (self.value & 0x01) != 0
    }

    /// Bit 1: Graphics mode enable (0 = text, 1 = graphics)
    pub fn is_graphics(&self) -> bool {
        (self.value & 0x02) != 0
    }

    /// Bit 2: Monochrome mode (0 = color, 1 = monochrome)
    pub fn is_monochrome(&self) -> bool {
        (self.value & 0x04) != 0
    }

    /// Bit 3: Video enable (0 = disabled, 1 = enabled)
    pub fn is_video_enabled(&self) -> bool {
        (self.value & 0x08) != 0
    }

    /// Bit 4: High-resolution graphics (0 = 320x200, 1 = 640x200)
    pub fn is_high_res(&self) -> bool {
        (self.value & 0x10) != 0
    }

    /// Bit 5: Blink enable (0 = intensity, 1 = blink)
    pub fn is_blink_enabled(&self) -> bool {
        (self.value & 0x20) != 0
    }

    pub fn write(&mut self, value: u8) {
        self.value = value;
    }

    pub fn read(&self) -> u8 {
        self.value
    }
}

impl Default for CgaModeControl {
    fn default() -> Self {
        Self::new()
    }
}

/// CGA Color Select Register (port 0x3D9)
/// Handled directly by Video::set_palette()
```

**File**: `core/src/io/mod.rs` (MODIFY)

Integrate CGA ports:

```rust
mod cga_ports;
use cga_ports::CgaModeControl;

pub struct IoDevice {
    last_write: HashMap<u16, u8>,
    system_control_port: SystemControlPort,
    cga_mode_control: CgaModeControl,
}

impl IoDevice {
    pub fn new() -> Self {
        Self {
            last_write: HashMap::new(),
            system_control_port: SystemControlPort::new(),
            cga_mode_control: CgaModeControl::new(),
        }
    }

    pub fn read_byte(&mut self, port: u16) -> u8 {
        match port {
            // ... existing ports ...
            0x3D8 => self.cga_mode_control.read(),
            0x3D9 => 0xFF, // Color select is write-only
            // ... rest ...
            _ => {
                log::warn!("I/O Read from unimplemented port: 0x{:04X}", port);
                0xFF
            }
        }
    }

    pub fn write_byte(&mut self, port: u16, value: u8, video: &mut Video) {
        self.last_write.insert(port, value);

        match port {
            // ... existing ports ...
            0x3D8 => {
                self.cga_mode_control.write(value);
                log::debug!("CGA Mode Control: 0x{:02X}", value);
            }
            0x3D9 => {
                video.set_palette(value);
                log::debug!("CGA Color Select: 0x{:02X}", value);
            }
            // ... rest ...
        }
    }
}
```

### Phase 4: Computer Integration

**File**: `core/src/computer.rs` (MODIFY)

Update step() to render graphics:

```rust
impl<K: KeyboardInput, V: VideoController> Computer<K, V> {
    pub fn step(&mut self) -> Result<bool> {
        // ... existing instruction execution ...

        // Update video controller based on current mode
        if self.video.is_dirty() {
            match self.video.get_mode_type() {
                VideoMode::Text { .. } => {
                    self.video_controller.update_display(self.video.get_buffer());
                }
                VideoMode::Graphics320x200 => {
                    if let Some(buffer) = self.video.get_graphics_buffer() {
                        self.video_controller.update_graphics_320x200(
                            buffer.get_pixels(),
                            self.video.get_palette(),
                        );
                    }
                }
                VideoMode::Graphics640x200 => {
                    if let Some(buffer) = self.video.get_graphics_buffer() {
                        let palette = self.video.get_palette();
                        let colors = palette.get_colors();
                        self.video_controller.update_graphics_640x200(
                            buffer.get_pixels(),
                            colors[1], // Foreground
                            colors[0], // Background
                        );
                    }
                }
            }
            self.video.clear_dirty();
        }

        // ... rest of step() ...
    }
}
```

### Phase 5: INT 10h Graphics Functions

**File**: `core/src/cpu/bios/int10.rs` (MODIFY)

Add graphics-specific functions:

```rust
impl Cpu {
    fn handle_int10(&mut self, memory: &mut Memory, video: &mut Video) {
        let function = (self.ax >> 8) as u8;

        match function {
            // ... existing functions ...
            0x0C => self.int10_write_pixel(video),
            0x0D => self.int10_read_pixel(video),
            // ... rest ...
        }
    }

    /// INT 10h, AH=0Ch - Write Graphics Pixel
    /// Input:
    ///   AL = pixel color value (0-3 for 320x200, 0-1 for 640x200)
    ///   BH = page number (0 for graphics modes)
    ///   CX = column (0-319 or 0-639)
    ///   DX = row (0-199)
    /// Output: None
    fn int10_write_pixel(&mut self, video: &mut Video) {
        let color = (self.ax & 0xFF) as u8; // AL
        let col = self.cx as usize;
        let row = self.dx as usize;

        match video.get_mode_type() {
            VideoMode::Graphics320x200 => {
                if col >= 320 || row >= 200 {
                    return;
                }
                // Calculate byte offset (4 pixels per byte)
                let pixels_per_byte = 4;
                let bytes_per_line = 320 / pixels_per_byte; // 80
                let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                let pixel_in_byte = col % pixels_per_byte;

                // Read-modify-write
                let mut byte_val = video.read_byte(byte_offset);
                let shift = 6 - (pixel_in_byte * 2); // MSB first
                byte_val &= !(0x03 << shift); // Clear 2 bits
                byte_val |= (color & 0x03) << shift; // Set 2 bits
                video.write_byte(byte_offset, byte_val);
            }
            VideoMode::Graphics640x200 => {
                if col >= 640 || row >= 200 {
                    return;
                }
                // Calculate byte offset (8 pixels per byte)
                let pixels_per_byte = 8;
                let bytes_per_line = 640 / pixels_per_byte; // 80
                let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                let pixel_in_byte = col % pixels_per_byte;

                // Read-modify-write
                let mut byte_val = video.read_byte(byte_offset);
                let bit_mask = 0x80 >> pixel_in_byte; // MSB first
                if (color & 0x01) != 0 {
                    byte_val |= bit_mask; // Set bit
                } else {
                    byte_val &= !bit_mask; // Clear bit
                }
                video.write_byte(byte_offset, byte_val);
            }
            VideoMode::Text { .. } => {
                // Ignore write pixel in text mode
            }
        }
    }

    /// INT 10h, AH=0Dh - Read Graphics Pixel
    /// Input:
    ///   BH = page number (0 for graphics modes)
    ///   CX = column
    ///   DX = row
    /// Output:
    ///   AL = pixel color value
    fn int10_read_pixel(&mut self, video: &Video) {
        let col = self.cx as usize;
        let row = self.dx as usize;

        let color = match video.get_mode_type() {
            VideoMode::Graphics320x200 => {
                if col >= 320 || row >= 200 {
                    0
                } else {
                    let pixels_per_byte = 4;
                    let bytes_per_line = 80;
                    let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                    let pixel_in_byte = col % pixels_per_byte;

                    let byte_val = video.read_byte(byte_offset);
                    let shift = 6 - (pixel_in_byte * 2);
                    (byte_val >> shift) & 0x03
                }
            }
            VideoMode::Graphics640x200 => {
                if col >= 640 || row >= 200 {
                    0
                } else {
                    let pixels_per_byte = 8;
                    let bytes_per_line = 80;
                    let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                    let pixel_in_byte = col % pixels_per_byte;

                    let byte_val = video.read_byte(byte_offset);
                    let bit_mask = 0x80 >> pixel_in_byte;
                    if (byte_val & bit_mask) != 0 { 1 } else { 0 }
                }
            }
            VideoMode::Text { .. } => 0,
        };

        self.ax = (self.ax & 0xFF00) | (color as u16);
    }
}
```

### Phase 6: Native Terminal Graphics (ASCII Art Fallback)

**File**: `native/src/terminal_video.rs` (MODIFY)

Add graphics rendering using block characters:

```rust
impl VideoController for TerminalVideo {
    // ... existing text mode methods ...

    fn update_graphics_320x200(&mut self, pixel_data: &[u8], palette: &CgaPalette) {
        // Use Unicode block characters to approximate graphics
        // Each terminal character represents 2x4 pixels (8 pixels total)
        // This gives us ~160x50 effective resolution in terminal

        let colors = palette.get_colors();
        let width = 320;
        let height = 200;

        // Render 2 pixel rows at a time using half-block characters
        for term_row in 0..100 {
            for term_col in 0..160 {
                let px_col = term_col * 2;
                let px_row = term_row * 2;

                // Get colors of 2x2 pixel block
                let top_left = self.get_pixel_color_320(pixel_data, px_col, px_row);
                let top_right = self.get_pixel_color_320(pixel_data, px_col + 1, px_row);
                let bot_left = self.get_pixel_color_320(pixel_data, px_col, px_row + 1);
                let bot_right = self.get_pixel_color_320(pixel_data, px_col + 1, px_row + 1);

                // Use half-block character: '▀' (top half) or '▄' (bottom half)
                // This is a simplified approximation
                let (ch, fg, bg) = if top_left == top_right && bot_left == bot_right {
                    if top_left == bot_left {
                        // All same color: space
                        (' ', colors[top_left as usize], colors[top_left as usize])
                    } else {
                        // Top vs bottom: use half-block
                        ('▀', colors[top_left as usize], colors[bot_left as usize])
                    }
                } else {
                    // Complex case: pick dominant color
                    // Simplified: just use top-left
                    (' ', colors[top_left as usize], colors[top_left as usize])
                };

                // Print character with ANSI colors
                print!("{}", Self::ansi_color_fg(fg));
                print!("{}", Self::ansi_color_bg(bg));
                print!("{}", ch);
            }
            println!("{}", Self::ansi_reset());
        }
    }

    fn update_graphics_640x200(&mut self, pixel_data: &[u8], fg_color: u8, bg_color: u8) {
        // Similar approach but for 1-bit pixels
        // 640x200 -> 320x100 terminal resolution with half-blocks
        log::warn!("640x200 graphics mode rendering not fully implemented in terminal");
    }

    // Helper methods
    fn get_pixel_color_320(&self, data: &[u8], x: usize, y: usize) -> u8 {
        if x >= 320 || y >= 200 {
            return 0;
        }
        let byte_offset = y * 80 + x / 4;
        let pixel_in_byte = x % 4;
        let byte_val = data[byte_offset];
        let shift = 6 - (pixel_in_byte * 2);
        (byte_val >> shift) & 0x03
    }

    fn ansi_color_fg(color: u8) -> String {
        // Convert VGA color to ANSI escape code
        match color {
            0 => "\x1b[30m",   // Black
            1 => "\x1b[34m",   // Blue
            2 => "\x1b[32m",   // Green
            3 => "\x1b[36m",   // Cyan
            4 => "\x1b[31m",   // Red
            5 => "\x1b[35m",   // Magenta
            6 => "\x1b[33m",   // Brown/Yellow
            7 => "\x1b[37m",   // Light gray
            8 => "\x1b[90m",   // Dark gray
            9 => "\x1b[94m",   // Light blue
            10 => "\x1b[92m",  // Light green
            11 => "\x1b[96m",  // Light cyan
            12 => "\x1b[91m",  // Light red
            13 => "\x1b[95m",  // Light magenta
            14 => "\x1b[93m",  // Yellow
            15 => "\x1b[97m",  // White
            _ => "\x1b[37m",
        }.to_string()
    }

    fn ansi_color_bg(color: u8) -> String {
        // Similar mapping but for background (40-47, 100-107)
        match color {
            0 => "\x1b[40m",   // Black
            1 => "\x1b[44m",   // Blue
            // ... (similar pattern as foreground but with +10 to escape code)
            _ => "\x1b[40m",
        }.to_string()
    }

    fn ansi_reset() -> &'static str {
        "\x1b[0m"
    }
}
```

**Note**: Terminal graphics will be low-fidelity. True graphics experience requires GUI implementation.

### Phase 7: GUI Graphics Rendering (native-gui)

**File**: `native-gui/src/gui_video.rs` (MODIFY)

Implement proper pixel rendering:

```rust
impl VideoController for GuiVideo {
    // ... existing text mode methods ...

    fn update_graphics_320x200(&mut self, pixel_data: &[u8], palette: &CgaPalette) {
        let colors = palette.get_colors();
        let scale = 2; // Scale factor: 320x200 -> 640x400 window

        // Iterate through all pixels
        for y in 0..200 {
            for x in 0..320 {
                // Extract pixel color (2 bits per pixel, 4 pixels per byte)
                let byte_offset = y * 80 + x / 4;
                let pixel_in_byte = x % 4;
                let byte_val = pixel_data[byte_offset];
                let shift = 6 - (pixel_in_byte * 2);
                let color_index = ((byte_val >> shift) & 0x03) as usize;

                // Get RGB color from palette
                let vga_color = colors[color_index];
                let rgb = self.vga_to_rgb(vga_color);

                // Draw scaled pixel (2x2 screen pixels per CGA pixel)
                for dy in 0..scale {
                    for dx in 0..scale {
                        let screen_x = x * scale + dx;
                        let screen_y = y * scale + dy;
                        self.framebuffer.put_pixel(screen_x as u32, screen_y as u32, rgb);
                    }
                }
            }
        }

        // Update window
        self.window.update_with_buffer(&self.framebuffer, 640, 400).unwrap();
    }

    fn update_graphics_640x200(&mut self, pixel_data: &[u8], fg_color: u8, bg_color: u8) {
        let fg_rgb = self.vga_to_rgb(fg_color);
        let bg_rgb = self.vga_to_rgb(bg_color);
        let scale = 2; // 640x200 -> 1280x400

        for y in 0..200 {
            for x in 0..640 {
                let byte_offset = y * 80 + x / 8;
                let pixel_in_byte = x % 8;
                let byte_val = pixel_data[byte_offset];
                let bit_mask = 0x80 >> pixel_in_byte;
                let is_set = (byte_val & bit_mask) != 0;

                let rgb = if is_set { fg_rgb } else { bg_rgb };

                for dy in 0..scale {
                    for dx in 0..scale {
                        let screen_x = x * scale + dx;
                        let screen_y = y * scale + dy;
                        self.framebuffer.put_pixel(screen_x as u32, screen_y as u32, rgb);
                    }
                }
            }
        }

        self.window.update_with_buffer(&self.framebuffer, 1280, 400).unwrap();
    }

    fn vga_to_rgb(&self, color: u8) -> Rgb {
        // Convert VGA color index to RGB
        match color {
            0 => Rgb(0, 0, 0),         // Black
            1 => Rgb(0, 0, 170),       // Blue
            2 => Rgb(0, 170, 0),       // Green
            3 => Rgb(0, 170, 170),     // Cyan
            4 => Rgb(170, 0, 0),       // Red
            5 => Rgb(170, 0, 170),     // Magenta
            6 => Rgb(170, 85, 0),      // Brown
            7 => Rgb(170, 170, 170),   // Light gray
            8 => Rgb(85, 85, 85),      // Dark gray
            9 => Rgb(85, 85, 255),     // Light blue
            10 => Rgb(85, 255, 85),    // Light green
            11 => Rgb(85, 255, 255),   // Light cyan
            12 => Rgb(255, 85, 85),    // Light red
            13 => Rgb(255, 85, 255),   // Light magenta
            14 => Rgb(255, 255, 85),   // Yellow
            15 => Rgb(255, 255, 255),  // White
            _ => Rgb(0, 0, 0),
        }
    }
}
```

### Phase 8: WASM Graphics Rendering

**File**: `wasm/src/web_video.rs` (MODIFY - referenced in plans/02-wasm.md)

Update WASM video controller for CGA:

```rust
impl VideoController for WebVideo {
    // ... existing text mode methods ...

    fn update_graphics_320x200(&mut self, pixel_data: &[u8], palette: &CgaPalette) {
        // Resize canvas for graphics mode
        self.canvas.set_width(640);  // 320 * 2 (scaled)
        self.canvas.set_height(400); // 200 * 2 (scaled)

        let colors = palette.get_colors();

        // Create ImageData for fast pixel rendering
        let width = 320;
        let height = 200;
        let mut image_data_buf = vec![0u8; width * height * 4]; // RGBA

        for y in 0..height {
            for x in 0..width {
                let byte_offset = y * 80 + x / 4;
                let pixel_in_byte = x % 4;
                let byte_val = pixel_data[byte_offset];
                let shift = 6 - (pixel_in_byte * 2);
                let color_index = ((byte_val >> shift) & 0x03) as usize;

                let rgb = Self::vga_to_rgb(colors[color_index]);
                let pixel_offset = (y * width + x) * 4;
                image_data_buf[pixel_offset] = rgb.0;     // R
                image_data_buf[pixel_offset + 1] = rgb.1; // G
                image_data_buf[pixel_offset + 2] = rgb.2; // B
                image_data_buf[pixel_offset + 3] = 255;   // A
            }
        }

        // Create ImageData and render to canvas (scaled 2x)
        let image_data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&image_data_buf),
            width as u32,
            height as u32,
        ).unwrap();

        // Draw scaled to canvas
        self.context.put_image_data(&image_data, 0.0, 0.0).ok();
        self.context.scale(2.0, 2.0).ok();
    }

    fn update_graphics_640x200(&mut self, pixel_data: &[u8], fg_color: u8, bg_color: u8) {
        // Similar implementation for 640x200 monochrome mode
        log::warn!("640x200 graphics mode not yet fully implemented in WASM");
    }

    fn vga_to_rgb(color: u8) -> (u8, u8, u8) {
        match color {
            0 => (0, 0, 0),         // Black
            1 => (0, 0, 170),       // Blue
            2 => (0, 170, 0),       // Green
            // ... (same mapping as GUI version)
            _ => (0, 0, 0),
        }
    }
}
```

## Mode Switching Flow

### Text to Graphics Transition

1. **DOS program executes**: `INT 10h, AH=00h, AL=04h` (set mode 0x04)
2. **INT 10h handler**: Calls `video.set_mode(0x04)`
3. **Video::set_mode()**:
   - Sets `mode = 0x04`
   - Sets `mode_type = VideoMode::Graphics320x200`
   - Allocates `graphics_buffer = Some(GraphicsBuffer::new_320x200())`
   - Clears graphics buffer to black
   - Deallocates text buffer (or keeps for fast switching)
   - Sets `dirty = true`
4. **Next step()**: Detects graphics mode, calls `video_controller.update_graphics_320x200()`
5. **Platform renderer**: Switches to pixel rendering mode

### Graphics to Text Transition

1. **DOS program executes**: `INT 10h, AH=00h, AL=03h` (set mode 0x03)
2. **INT 10h handler**: Calls `video.set_mode(0x03)`
3. **Video::set_mode()**:
   - Sets `mode = 0x03`
   - Sets `mode_type = VideoMode::Text { cols: 80, rows: 25 }`
   - Deallocates `graphics_buffer = None`
   - Clears text buffer
   - Sets `dirty = true`
4. **Next step()**: Detects text mode, calls `video_controller.update_display()`
5. **Platform renderer**: Switches back to text rendering

### Direct Video Memory Access

DOS programs can also write directly to video memory (0xB8000):

1. **Program writes**: `MOV BYTE PTR ES:[BX], AL` where ES=0xB800
2. **Memory write handler**: Detects video memory range, calls `video.write_byte(offset, value)`
3. **Video::write_byte()**:
   - Determines current mode (text vs graphics)
   - If graphics: applies interlaced offset conversion
   - Writes to appropriate buffer
   - Sets `dirty = true`
4. **Next step()**: Renders updated buffer

## Critical Files Summary

| File | Action | Purpose |
|------|--------|---------|
| `core/src/video.rs` | MODIFY | Add GraphicsBuffer, CgaPalette, VideoMode enum, graphics read/write |
| `core/src/io/cga_ports.rs` | CREATE | CGA Mode Control Register (0x3D8) handler |
| `core/src/io/mod.rs` | MODIFY | Route CGA ports 0x3D8, 0x3D9 |
| `core/src/computer.rs` | MODIFY | Update step() to handle graphics rendering |
| `core/src/cpu/bios/int10.rs` | MODIFY | Add INT 10h AH=0Ch (write pixel), AH=0Dh (read pixel) |
| `native/src/terminal_video.rs` | MODIFY | Add ASCII art graphics rendering (low fidelity) |
| `native-gui/src/gui_video.rs` | MODIFY | Add proper pixel rendering for graphics modes |
| `wasm/src/web_video.rs` | MODIFY | Add canvas-based graphics rendering |

## Updates to Other Plans

### plans/02-wasm.md

**Section to Update**: "Future Enhancements" > "CGA Graphics Mode"

Replace:
```markdown
### CGA Graphics Mode
When CGA is implemented, render pixel graphics to canvas (320x200 or 640x200).
```

With:
```markdown
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
```

## Verification Strategy

### Test 1: Mode Switch (Text -> Graphics -> Text)
```nasm
; mode_switch.asm
org 0x100

; Start in text mode, write message
mov ah, 0x09
mov dx, msg_text
int 0x21

; Wait for key
mov ah, 0x00
int 0x16

; Switch to 320x200 graphics
mov ah, 0x00
mov al, 0x04
int 0x10

; Draw some pixels
mov ah, 0x0C  ; Write pixel
mov al, 1     ; Color 1
mov cx, 160   ; X = center
mov dx, 100   ; Y = center
int 0x10

; Wait for key
mov ah, 0x00
int 0x16

; Back to text mode
mov ah, 0x00
mov al, 0x03
int 0x10

; Write message
mov ah, 0x09
mov dx, msg_back
int 0x21

; Exit
mov ah, 0x4C
int 0x21

msg_text db 'Text mode. Press key for graphics.$'
msg_back db 'Back to text mode.$'
```

**Expected**: Text message, graphics pixel, back to text message

### Test 2: Palette Changes
```nasm
; palette.asm - Cycle through CGA palettes
org 0x100

mov ah, 0x00
mov al, 0x04  ; 320x200 graphics
int 0x10

; Fill screen with pattern
call draw_pattern

; Cycle through palettes
mov cx, 4
.palette_loop:
    ; Wait
    call delay

    ; Change palette
    mov dx, 0x3D9
    mov al, cl
    shl al, 5     ; Palette in bit 5
    or al, 0x00   ; Black background
    out dx, al

    loop .palette_loop

; Exit
mov ah, 0x4C
int 0x21

draw_pattern:
    ; Draw vertical stripes of each color
    ; ... (implementation)
    ret

delay:
    ; ... (implementation)
    ret
```

**Expected**: Screen pattern changes colors as palette cycles

### Test 3: Direct Memory Write
```nasm
; direct_write.asm - Write directly to CGA memory
org 0x100

mov ah, 0x00
mov al, 0x04  ; 320x200 graphics
int 0x10

; Point ES to video memory
mov ax, 0xB800
mov es, ax

; Fill first line with pattern
xor di, di
mov cx, 80    ; 80 bytes = 320 pixels
mov al, 0xAA  ; Pattern: 10101010 (colors 2,2,2,2)
rep stosb

; Fill line 1 (odd bank, offset 0x2000)
mov di, 0x2000
mov cx, 80
mov al, 0x55  ; Pattern: 01010101 (colors 1,1,1,1)
rep stosb

; Exit
mov ah, 0x4C
int 0x21
```

**Expected**: Two horizontal lines with different color patterns

### Test 4: Read Pixel
```nasm
; read_pixel.asm - Verify pixel read/write
org 0x100

mov ah, 0x00
mov al, 0x04
int 0x10

; Write pixel
mov ah, 0x0C
mov al, 3     ; Color 3
mov cx, 100
mov dx, 100
int 0x10

; Read it back
mov ah, 0x0D
mov cx, 100
mov dx, 100
int 0x10

; AL should now contain 3
cmp al, 3
je .success

; Print error
mov ah, 0x00
mov al, 0x03  ; Back to text
int 0x10
mov ah, 0x09
mov dx, msg_fail
int 0x21
jmp .exit

.success:
mov ah, 0x00
mov al, 0x03
int 0x10
mov ah, 0x09
mov dx, msg_ok
int 0x21

.exit:
mov ah, 0x4C
int 0x21

msg_ok db 'Pixel read OK!$'
msg_fail db 'Pixel read FAILED!$'
```

**Expected**: "Pixel read OK!" message

### Test 5: Real CGA Programs
- **GORILLAS.BAS** (QBasic): Gorilla throwing game (320x200 graphics)
- **NIBBLES.BAS** (QBasic): Snake game (text mode, good for regression)
- **PC Paint**: Simple graphics program
- **Early DOS Games**: Alley Cat, Digger, etc.

## Implementation Notes

### CGA Interlaced Memory

Critical for compatibility! CGA hardware uses interlaced memory:
- **Even scan lines** (0, 2, 4, ...): Bank 0 at 0xB8000-0xB9F3F
- **Odd scan lines** (1, 3, 5, ...): Bank 1 at 0xBA000-0xBBF3F

Programs writing to 0xB8000 expect to write to line 0, not line 0 and 1.

### Pixel Packing

**320x200 mode**:
- 4 pixels per byte
- Byte format: `[px0:2][px1:2][px2:2][px3:2]` where px0 is leftmost (MSB first)
- Example: `0b11100100` = pixels [3, 2, 1, 0]

**640x200 mode**:
- 8 pixels per byte
- Byte format: `[px0][px1][px2][px3][px4][px5][px6][px7]` (MSB first)
- Example: `0b10101010` = alternating on/off pixels

### Performance Considerations

**Native GUI**: Direct pixel rendering to framebuffer is fast
**WASM**: Use ImageData API for efficient bulk pixel updates
**Terminal**: ASCII art is inherently low-fidelity, but functional for testing

### Future Enhancements

**EGA Support**:
- 16-color modes (0x0D, 0x0E, 0x10)
- Planar memory architecture (different from CGA)
- Extended palette registers

**VGA Support**:
- Mode 0x13 (320x200, 256 colors)
- Linear framebuffer (easier than CGA!)
- DAC palette with 262144 colors

**Composite Color Artifacts**:
- CGA composite output creates additional "artifact colors"
- Used by some games for 16-color effect
- Complex to emulate, low priority

## Testing Commands

```bash
# Build all crates
cargo build

# Run mode switch test
./examples/run.sh mode_switch

# Run in GUI with graphics support
cargo run -p emu86-native-gui -- --boot --floppy-a test_graphics.img

# Run in WASM (after implementing Phase 8)
cd wasm && ./build.sh && python3 -m http.server --directory www 8080

# Check logs for mode changes
tail -f emu86.log | grep -E "(Video mode|CGA|Graphics)"
```

## Success Criteria

- [ ] Video struct supports both text and graphics modes
- [ ] Mode 0x04 (320x200, 4-color) renders correctly
- [ ] Mode 0x06 (640x200, 2-color) renders correctly
- [ ] CGA interlaced memory layout is accurate
- [ ] Palette changes via port 0x3D9 work correctly
- [ ] INT 10h AH=0Ch (write pixel) functions correctly
- [ ] INT 10h AH=0Dh (read pixel) returns correct values
- [ ] Direct video memory writes render properly
- [ ] Mode switching (text ↔ graphics) works seamlessly
- [ ] Terminal ASCII art rendering works (low fidelity)
- [ ] GUI native pixel rendering works (high fidelity)
- [ ] WASM canvas rendering works
- [ ] Real CGA programs (GORILLAS.BAS, etc.) run correctly
- [ ] No regression in existing text mode functionality
