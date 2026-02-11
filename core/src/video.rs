// Video memory constants
pub const VIDEO_MEMORY_START: usize = 0xB8000;
pub const VIDEO_MEMORY_END: usize = 0xBFFFF;
pub const VIDEO_MEMORY_SIZE: usize = VIDEO_MEMORY_END - VIDEO_MEMORY_START + 1; // 32KB

// Text mode dimensions
pub const TEXT_MODE_COLS: usize = 80;
pub const TEXT_MODE_ROWS: usize = 25;
pub const TEXT_MODE_BUFFER_SIZE: usize = TEXT_MODE_COLS * TEXT_MODE_ROWS * 2; // char + attr

// VGA color constants
#[allow(dead_code)]
pub mod colors {
    pub const BLACK: u8 = 0x0;
    pub const BLUE: u8 = 0x1;
    pub const GREEN: u8 = 0x2;
    pub const CYAN: u8 = 0x3;
    pub const RED: u8 = 0x4;
    pub const MAGENTA: u8 = 0x5;
    pub const BROWN: u8 = 0x6;
    pub const LIGHT_GRAY: u8 = 0x7;
    pub const DARK_GRAY: u8 = 0x8;
    pub const LIGHT_BLUE: u8 = 0x9;
    pub const LIGHT_GREEN: u8 = 0xA;
    pub const LIGHT_CYAN: u8 = 0xB;
    pub const LIGHT_RED: u8 = 0xC;
    pub const LIGHT_MAGENTA: u8 = 0xD;
    pub const YELLOW: u8 = 0xE;
    pub const WHITE: u8 = 0xF;
}

/// VGA text mode character attribute
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextAttribute {
    pub foreground: u8, // 4 bits
    pub background: u8, // 3 bits
    pub blink: bool,    // 1 bit
}

impl TextAttribute {
    /// Create from VGA attribute byte
    pub fn from_byte(byte: u8) -> Self {
        Self {
            foreground: byte & 0x0F,
            background: (byte >> 4) & 0x07,
            blink: (byte & 0x80) != 0,
        }
    }

    /// Convert to VGA attribute byte
    pub fn to_byte(&self) -> u8 {
        let mut byte = self.foreground & 0x0F;
        byte |= (self.background & 0x07) << 4;
        if self.blink {
            byte |= 0x80;
        }
        byte
    }
}

impl Default for TextAttribute {
    fn default() -> Self {
        Self {
            foreground: colors::LIGHT_GRAY,
            background: colors::BLACK,
            blink: false,
        }
    }
}

/// A single character cell in text mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCell {
    pub character: u8,
    pub attribute: TextAttribute,
}

impl Default for TextCell {
    fn default() -> Self {
        Self {
            character: 0x20, // Space character
            attribute: TextAttribute::default(),
        }
    }
}

/// Cursor position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorPosition {
    pub row: usize,
    pub col: usize,
}

/// Video mode type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoMode {
    /// Text modes: 80x25 or 40x25
    Text { cols: usize, rows: usize },
    /// CGA 320x200, 4 colors
    Graphics320x200,
    /// CGA 640x200, 2 colors
    Graphics640x200,
    /// EGA 320x200, 16 colors (mode 0x0D)
    Graphics320x200x16,
}

/// Graphics framebuffer for CGA modes
pub struct GraphicsBuffer {
    /// Raw pixel data (16KB for CGA modes)
    /// Interlaced: first 8KB = even scan lines, second 8KB = odd scan lines
    data: Vec<u8>,
    /// Width in pixels
    width: usize,
    /// Height in pixels
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    fn linear_to_interlaced(&self, offset: usize) -> usize {
        let bytes_per_line = self.width * (self.bits_per_pixel as usize) / 8;
        let line = offset / bytes_per_line;
        let col = offset % bytes_per_line;

        if line.is_multiple_of(2) {
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
        // Convert from interlaced CGA address to linear offset
        let linear_offset = self.interlaced_to_linear(offset);
        if linear_offset >= self.data.len() {
            return 0;
        }
        self.data[linear_offset]
    }

    /// Write byte to graphics memory (using interlaced addressing)
    pub fn write_byte(&mut self, offset: usize, value: u8) {
        // Debug logging for graphics writes
        if value != 0 {
            let bytes_per_line = self.width * (self.bits_per_pixel as usize) / 8;
            let (y, x_byte) = if offset < 0x2000 {
                let line_in_bank = offset / bytes_per_line;
                (line_in_bank * 2, offset % bytes_per_line)
            } else {
                let offset_in_bank = offset - 0x2000;
                let line_in_bank = offset_in_bank / bytes_per_line;
                (line_in_bank * 2 + 1, offset_in_bank % bytes_per_line)
            };
            let pixels_per_byte = 8 / self.bits_per_pixel as usize;
            let x = x_byte * pixels_per_byte;
            log::debug!(
                "Graphics write: offset=0x{:04X} (x={}, y={}), value=0x{:02X} ({}bpp)",
                offset,
                x,
                y,
                value,
                self.bits_per_pixel
            );
        }

        // Convert from interlaced CGA address to linear offset
        let linear_offset = self.interlaced_to_linear(offset);
        if linear_offset >= self.data.len() {
            return;
        }

        self.data[linear_offset] = value;
    }

    /// Get pixel data as linear buffer (for rendering)
    pub fn get_pixels(&self) -> &[u8] {
        &self.data
    }
}

/// EGA planar framebuffer for mode 0x0D (320x200 16-color)
/// 4 bit planes, each 8000 bytes (320*200/8 pixels per byte)
pub struct EgaBuffer {
    /// 4 bit planes, each 8000 bytes
    planes: [[u8; 8000]; 4],
}

impl EgaBuffer {
    pub fn new() -> Self {
        Self {
            planes: [[0u8; 8000]; 4],
        }
    }

    /// Write a byte to all enabled planes (map_mask = bitmask of planes 0-3)
    pub fn write_byte(&mut self, map_mask: u8, offset: usize, value: u8) {
        if offset >= 8000 {
            return;
        }
        for plane in 0..4 {
            if map_mask & (1 << plane) != 0 {
                self.planes[plane][offset] = value;
            }
        }
    }

    /// Read a byte from the selected plane
    pub fn read_byte(&self, read_plane: u8, offset: usize) -> u8 {
        let plane = (read_plane & 3) as usize;
        if offset >= 8000 {
            return 0;
        }
        self.planes[plane][offset]
    }

    /// Compose 16-color pixel data (320*200 = 64000 bytes, each byte is a 0-15 color index)
    pub fn get_pixels(&self) -> Vec<u8> {
        let mut pixels = vec![0u8; 320 * 200];
        for y in 0..200usize {
            for x in 0..320usize {
                let byte_offset = y * 40 + x / 8;
                let bit = 7 - (x % 8);
                let color = ((self.planes[0][byte_offset] >> bit) & 1)
                    | (((self.planes[1][byte_offset] >> bit) & 1) << 1)
                    | (((self.planes[2][byte_offset] >> bit) & 1) << 2)
                    | (((self.planes[3][byte_offset] >> bit) & 1) << 3);
                pixels[y * 320 + x] = color;
            }
        }
        pixels
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
            // Palette 0 (bit 5 = 0): Green, Red, Brown
            if self.intensity {
                [bg, colors::LIGHT_GREEN, colors::LIGHT_RED, colors::YELLOW]
            } else {
                // Use actual CGA hardware colors for accuracy
                [bg, colors::GREEN, colors::RED, colors::BROWN]
            }
        } else {
            // Palette 1 (bit 5 = 1): Cyan, Magenta, Light Gray/White
            if self.intensity {
                [bg, colors::LIGHT_CYAN, colors::LIGHT_MAGENTA, colors::WHITE]
            } else {
                // Use actual CGA hardware color (Light Gray)
                // On period monitors this appeared bright/white-ish
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

/// Video controller trait - platform-specific implementations provide rendering
pub trait VideoController {
    /// Called when video memory is updated
    /// Provides the entire text buffer for rendering
    fn update_display(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]);

    /// Update cursor position
    fn update_cursor(&mut self, position: CursorPosition);

    /// Called on mode changes (future: support multiple video modes)
    fn set_video_mode(&mut self, mode: u8);

    /// Force a full redraw of the entire screen, ignoring cached state
    /// Used when the terminal state is known to be out of sync (e.g., after clearing screen)
    fn force_redraw(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]);

    /// Update graphics display (320x200, 4 colors)
    /// pixel_data: linear pixel array
    /// cga_palette: 4 EGA color indices (0-15) from CGA palette [bg, color1, color2, color3]
    /// For CGA compatibility, pixel values 0-3 map to these EGA colors, not VGA DAC
    fn update_graphics_320x200(&mut self, pixel_data: &[u8], cga_palette: [u8; 4]) {
        // Default implementation: log warning
        let _ = (pixel_data, cga_palette);
        log::warn!("Graphics mode 320x200 not implemented for this platform");
    }

    /// Update graphics display (640x200, 2 colors)
    /// pixel_data: linear pixel array (1 bit per pixel), fg_color: foreground color
    fn update_graphics_640x200(&mut self, pixel_data: &[u8], fg_color: u8, bg_color: u8) {
        let _ = (pixel_data, fg_color, bg_color);
        log::warn!("Graphics mode 640x200 not implemented for this platform");
    }

    /// Update EGA graphics display (320x200, 16 colors, mode 0x0D)
    /// pixel_data: linear pixel array (320*200 bytes), each byte is a 0-15 color index
    fn update_graphics_320x200x16(&mut self, pixel_data: &[u8]) {
        let _ = pixel_data;
        log::warn!("Graphics mode 320x200x16 (EGA) not implemented for this platform");
    }

    /// Set cursor visibility
    fn set_cursor_visible(&mut self, visible: bool) {
        let _ = visible;
        // Default implementation: do nothing
    }

    /// Update VGA DAC palette (for text mode color rendering)
    /// palette: array of 256 RGB triplets, each component is 6-bit (0-63)
    fn update_vga_dac_palette(&mut self, palette: &[[u8; 3]; 256]) {
        let _ = palette;
        // Default implementation: do nothing
    }

    /// Set border color (overscan) - the area around the display
    /// Only affects text modes; in graphics modes this is not visible
    fn set_border_color(&mut self, color: u8) {
        let _ = color;
        // Default implementation: do nothing
    }
}

/// Null video controller (no display)
#[derive(Debug, Default)]
pub struct NullVideoController;

impl VideoController for NullVideoController {
    fn update_display(&mut self, _buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {}
    fn update_cursor(&mut self, _position: CursorPosition) {}
    fn set_video_mode(&mut self, _mode: u8) {}
    fn force_redraw(&mut self, _buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {}
    // Graphics methods use default trait implementations
}

/// Core video state management
pub struct Video {
    /// Current cursor position
    cursor: CursorPosition,
    /// Text mode buffer (parsed representation)
    buffer: [TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS],
    /// Graphics mode buffer (optional, allocated when in graphics mode)
    graphics_buffer: Option<GraphicsBuffer>,
    /// EGA planar graphics buffer (optional, allocated in EGA modes)
    ega_buffer: Option<EgaBuffer>,
    /// Current video mode
    mode: u8,
    /// Parsed video mode type
    mode_type: VideoMode,
    /// Active display page (0-7 for text modes)
    active_page: u8,
    /// CGA palette state (graphics mode only)
    palette: CgaPalette,
    /// Border color (overscan) for text modes (4 bits, 0-15)
    border_color: u8,
    /// Dirty flag to minimize unnecessary updates
    dirty: bool,
    /// Flag to track if mode changed (needs controller notification)
    mode_changed: bool,
    /// VGA DAC palette registers (256 entries, each with 6-bit RGB components)
    vga_dac_palette: [[u8; 3]; 256],
    /// EGA Sequencer Map Mask (register 2): bitmask of planes to write (default 0x0F = all)
    ega_map_mask: u8,
    /// EGA Graphics Controller Read Map Select (register 4): which plane to read (0-3)
    ega_read_plane: u8,
}

/// Initialize VGA DAC palette with EGA defaults
fn default_vga_palette() -> [[u8; 3]; 256] {
    let mut palette = [[0u8; 3]; 256];

    // Initialize first 16 colors with EGA defaults (6-bit RGB values 0-63)
    for (i, entry) in palette.iter_mut().enumerate().take(16) {
        *entry = crate::palette::TextModePalette::get_dac_color(i as u8);
    }

    // Registers 16-255 remain black (already zeroed)
    palette
}

impl Video {
    pub fn new() -> Self {
        Self {
            cursor: CursorPosition::default(),
            buffer: [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS],
            graphics_buffer: None,
            ega_buffer: None,
            mode: 0x03, // 80x25 text mode
            mode_type: VideoMode::Text {
                cols: TEXT_MODE_COLS,
                rows: TEXT_MODE_ROWS,
            },
            active_page: 0,
            palette: CgaPalette::new(),
            border_color: 0, // Black border by default
            dirty: false,
            mode_changed: false,
            vga_dac_palette: default_vga_palette(),
            ega_map_mask: 0x0F, // All 4 planes enabled
            ega_read_plane: 0,  // Read from plane 0
        }
    }

    /// Read a single byte from video memory
    pub fn read_byte(&self, offset: usize) -> u8 {
        match &self.mode_type {
            VideoMode::Text { cols, .. } => {
                // Text mode: handle different column widths
                let bytes_per_row = cols * 2; // 2 bytes per cell (char + attr)
                let max_offset = cols * TEXT_MODE_ROWS * 2;

                if offset >= max_offset {
                    return 0;
                }

                // Calculate row and column in the actual video mode
                let row = offset / bytes_per_row;
                let col = (offset % bytes_per_row) / 2;

                // Map to internal 80-column buffer
                let cell_index = row * TEXT_MODE_COLS + col;
                if cell_index >= self.buffer.len() {
                    return 0;
                }

                if offset.is_multiple_of(2) {
                    self.buffer[cell_index].character
                } else {
                    self.buffer[cell_index].attribute.to_byte()
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
            VideoMode::Graphics320x200x16 => {
                // EGA mode: B800 reads return 0 (EGA uses A000)
                0
            }
        }
    }

    /// Update a single byte in video memory
    pub fn write_byte(&mut self, offset: usize, value: u8) {
        match &self.mode_type {
            VideoMode::Text { cols, .. } => {
                // Text mode: handle different column widths
                let bytes_per_row = cols * 2; // 2 bytes per cell (char + attr)
                let max_offset = cols * TEXT_MODE_ROWS * 2;

                if offset >= max_offset {
                    return;
                }

                // Calculate row and column in the actual video mode
                let row = offset / bytes_per_row;
                let col = (offset % bytes_per_row) / 2;

                // Map to internal 80-column buffer
                let cell_index = row * TEXT_MODE_COLS + col;
                if cell_index >= self.buffer.len() {
                    return;
                }

                if offset.is_multiple_of(2) {
                    self.buffer[cell_index].character = value;
                } else {
                    self.buffer[cell_index].attribute = TextAttribute::from_byte(value);
                }
            }
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200 => {
                // Graphics mode
                if let Some(ref mut buffer) = self.graphics_buffer {
                    buffer.write_byte(offset, value);
                }
            }
            VideoMode::Graphics320x200x16 => {
                // EGA mode: B800 writes are ignored (EGA uses A000 via write_byte_ega)
            }
        }
        self.dirty = true;
    }

    /// Update a word (char + attr) in video memory
    pub fn write_word(&mut self, offset: usize, value: u16) {
        self.write_byte(offset, (value & 0xFF) as u8);
        self.write_byte(offset + 1, (value >> 8) as u8);
    }

    /// Get the current buffer for rendering
    pub fn get_buffer(&self) -> &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS] {
        &self.buffer
    }

    /// Check if display needs updating
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark as clean after rendering
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Set cursor position
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        if row < TEXT_MODE_ROWS && col < TEXT_MODE_COLS {
            self.cursor = CursorPosition { row, col };
        }
    }

    /// Get cursor position
    pub fn get_cursor(&self) -> CursorPosition {
        self.cursor
    }

    /// Set video mode
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
            0x0D => {
                self.ega_buffer = Some(EgaBuffer::new());
                VideoMode::Graphics320x200x16
            }
            _ => {
                log::warn!("Unsupported video mode 0x{:02X}, defaulting to text", mode);
                VideoMode::Text { cols: 80, rows: 25 }
            }
        };

        // Clear buffers on mode change
        if matches!(self.mode_type, VideoMode::Text { .. }) {
            self.buffer = [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS];
            self.graphics_buffer = None;
            self.ega_buffer = None;
        } else if matches!(self.mode_type, VideoMode::Graphics320x200x16) {
            self.graphics_buffer = None;
        }

        // Reset VGA DAC palette to defaults
        // This ensures programs that modify the palette don't leave the system
        // with invisible text (e.g., black text on black background)
        log::info!("VGA DAC: Resetting palette to defaults on mode change");
        self.vga_dac_palette = default_vga_palette();

        // Reset CGA palette to defaults
        // For graphics modes (0x04-0x06), IBM CGA BIOS initializes to palette 1
        // For text modes, use palette 0
        self.palette = if matches!(
            self.mode_type,
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200
        ) {
            CgaPalette {
                background: 0,
                palette_id: 1, // Palette 1 (Cyan/Magenta/Light Gray) for graphics
                intensity: false,
            }
        } else {
            CgaPalette::new() // Palette 0 for text modes and EGA
        };

        // Sync VGA DAC entries 0-3 from CGA palette (for 320x200 4-color mode)
        self.update_vga_dac_from_cga_palette();

        self.dirty = true;
        self.mode_changed = true; // Flag that controller needs notification
        log::info!("Video mode set to 0x{:02X} ({:?})", mode, self.mode_type);
    }

    /// Get current video mode
    pub fn get_mode(&self) -> u8 {
        self.mode
    }

    /// Set active display page
    pub fn set_active_page(&mut self, page: u8) {
        self.active_page = page;
        self.dirty = true;
    }

    /// Get active display page
    pub fn get_active_page(&self) -> u8 {
        self.active_page
    }

    /// Check if currently in graphics mode
    pub fn is_graphics_mode(&self) -> bool {
        !matches!(self.mode_type, VideoMode::Text { .. })
    }

    /// Get current mode type
    pub fn get_mode_type(&self) -> VideoMode {
        self.mode_type
    }

    /// Get the current column count for the active mode
    /// Returns character columns (40 for 320px modes, 80 for 640px modes)
    pub fn get_cols(&self) -> usize {
        match self.mode_type {
            VideoMode::Text { cols, .. } => cols,
            VideoMode::Graphics320x200 | VideoMode::Graphics320x200x16 => 40,
            VideoMode::Graphics640x200 => 80,
        }
    }

    /// Get the current row count for the active mode
    /// Returns character rows (25 for 200px CGA/EGA modes)
    pub fn get_rows(&self) -> usize {
        match self.mode_type {
            VideoMode::Text { rows, .. } => rows,
            VideoMode::Graphics320x200
            | VideoMode::Graphics640x200
            | VideoMode::Graphics320x200x16 => 25,
        }
    }

    /// Set CGA palette (from I/O port 0x3D9)
    pub fn set_palette(&mut self, value: u8) {
        self.palette = CgaPalette::from_register(value);
        self.update_vga_dac_from_cga_palette();
        self.dirty = true;
    }

    /// Get CGA palette register value
    pub fn get_palette_register(&self) -> u8 {
        self.palette.to_register()
    }

    /// Get graphics buffer (for rendering)
    pub fn get_graphics_buffer(&self) -> Option<&GraphicsBuffer> {
        self.graphics_buffer.as_ref()
    }

    /// Get EGA buffer (for rendering)
    pub fn get_ega_buffer(&self) -> Option<&EgaBuffer> {
        self.ega_buffer.as_ref()
    }

    /// Write a byte to EGA planar memory (A000 segment)
    /// Writes to all planes enabled by the current Map Mask register
    pub fn write_byte_ega(&mut self, offset: usize, value: u8) {
        if let Some(buf) = &mut self.ega_buffer {
            buf.write_byte(self.ega_map_mask, offset, value);
            self.dirty = true;
        }
    }

    /// Read a byte from EGA planar memory (A000 segment)
    /// Reads from the plane selected by the Read Map Select register
    pub fn read_byte_ega(&self, offset: usize) -> u8 {
        if let Some(buf) = &self.ega_buffer {
            buf.read_byte(self.ega_read_plane, offset)
        } else {
            0
        }
    }

    /// Read a byte from a specific EGA plane (ignores Read Map Select register)
    pub fn read_byte_ega_plane(&self, plane: u8, offset: usize) -> u8 {
        if let Some(buf) = &self.ega_buffer {
            buf.read_byte(plane & 3, offset)
        } else {
            0
        }
    }

    /// Set EGA Sequencer Map Mask (register 2): which planes receive writes
    pub fn set_ega_map_mask(&mut self, value: u8) {
        self.ega_map_mask = value & 0x0F;
    }

    /// Set EGA Graphics Controller Read Map Select (register 4): which plane to read
    pub fn set_ega_read_plane(&mut self, value: u8) {
        self.ega_read_plane = value & 0x03;
    }

    /// Get palette (for rendering)
    pub fn get_palette(&self) -> &CgaPalette {
        &self.palette
    }

    /// Set CGA background color (4 bits, 0-15)
    pub fn set_cga_background(&mut self, color: u8) {
        self.palette.background = color & 0x0F;
        self.update_vga_dac_from_cga_palette();
        self.dirty = true;
    }

    /// Set CGA intensity/bright mode
    pub fn set_cga_intensity(&mut self, enabled: bool) {
        self.palette.intensity = enabled;
        self.update_vga_dac_from_cga_palette();
        self.dirty = true;
    }

    /// Set CGA palette ID (0 or 1)
    pub fn set_cga_palette_id(&mut self, palette_id: u8) {
        self.palette.palette_id = palette_id & 0x01;
        self.update_vga_dac_from_cga_palette();
        self.dirty = true;
    }

    /// Update VGA DAC palette entries 0-3 to match current CGA palette
    /// This allows CGA palette selection (INT 10h AH=0Bh) to work alongside
    /// VGA DAC programming (INT 10h AH=10h)
    fn update_vga_dac_from_cga_palette(&mut self) {
        let cga_colors = self.palette.get_colors(); // [bg, color1, color2, color3] as EGA indices
        let default_palette = default_vga_palette();

        // Map each CGA color slot to its VGA DAC entry
        for (i, &ega_color) in cga_colors.iter().enumerate() {
            let rgb = default_palette[ega_color as usize];
            self.vga_dac_palette[i] = rgb;
        }

        log::debug!(
            "VGA DAC: Synced entries 0-3 from CGA palette (id={}, intensity={}, bg={})",
            self.palette.palette_id,
            self.palette.intensity,
            self.palette.background
        );
    }

    /// Set VGA DAC register (6-bit RGB values 0-63)
    pub fn set_vga_dac_register(&mut self, index: u8, red: u8, green: u8, blue: u8) {
        log::info!(
            "VGA DAC: Setting palette[{}] = RGB({}, {}, {}) [6-bit]",
            index,
            red & 0x3F,
            green & 0x3F,
            blue & 0x3F
        );
        self.vga_dac_palette[index as usize] = [red & 0x3F, green & 0x3F, blue & 0x3F];
        self.dirty = true;
    }

    /// Get VGA DAC palette (for rendering)
    pub fn get_vga_dac_palette(&self) -> &[[u8; 3]; 256] {
        &self.vga_dac_palette
    }

    /// Get individual VGA DAC register (6-bit RGB values 0-63)
    pub fn get_vga_dac_register(&self, index: u8) -> [u8; 3] {
        self.vga_dac_palette[index as usize]
    }

    /// Set border color (overscan) for text modes
    pub fn set_border_color(&mut self, color: u8) {
        self.border_color = color & 0x0F;
        self.dirty = true;
        log::debug!("Video: Border color set to {}", self.border_color);
    }

    /// Get border color (overscan) for text modes
    pub fn get_border_color(&self) -> u8 {
        self.border_color
    }

    /// Check if video mode changed and clear the flag
    pub fn take_mode_changed(&mut self) -> bool {
        let changed = self.mode_changed;
        self.mode_changed = false;
        changed
    }
}

impl Default for Video {
    fn default() -> Self {
        Self::new()
    }
}

/// VGA I/O port handler for cursor control
pub struct VgaIoPorts {
    crtc_index: u8,
    cursor_location_high: u8,
    cursor_location_low: u8,
}

impl VgaIoPorts {
    pub fn new() -> Self {
        Self {
            crtc_index: 0,
            cursor_location_high: 0,
            cursor_location_low: 0,
        }
    }

    /// Handle write to CRT controller index register (0x3D4)
    pub fn write_index(&mut self, value: u8) {
        self.crtc_index = value;
    }

    /// Handle write to CRT controller data register (0x3D5)
    /// Returns Some(CursorPosition) if cursor position was updated
    /// cols: current video mode column count (40 or 80)
    pub fn write_data(&mut self, value: u8, cols: usize) -> Option<CursorPosition> {
        match self.crtc_index {
            0x0E => {
                // Cursor location high byte
                self.cursor_location_high = value;
                None
            }
            0x0F => {
                // Cursor location low byte
                self.cursor_location_low = value;
                // Calculate cursor position using actual column count
                let offset =
                    ((self.cursor_location_high as u16) << 8) | (self.cursor_location_low as u16);
                let row = (offset as usize) / cols;
                let col = (offset as usize) % cols;
                Some(CursorPosition { row, col })
            }
            _ => None,
        }
    }

    /// Handle read from data register
    pub fn read_data(&self) -> u8 {
        match self.crtc_index {
            0x0E => self.cursor_location_high,
            0x0F => self.cursor_location_low,
            _ => 0xFF,
        }
    }
}

impl Default for VgaIoPorts {
    fn default() -> Self {
        Self::new()
    }
}
