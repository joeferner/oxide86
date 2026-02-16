use crate::video::{
    cga::{CgaBuffer, CgaPalette},
    ega::EgaBuffer,
    text::TextBuffer,
};
use crate::video_card_type::VideoCardType;

pub mod cga;
pub mod composite;
pub mod ega;
pub mod render;
pub mod text;

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

/// Video controller trait - platform-specific implementations provide rendering
pub trait VideoController {
    /// Called when video memory is updated
    /// Provides the entire text buffer for rendering
    fn update_display(&mut self, buffer: &TextBuffer);

    /// Update cursor position
    fn update_cursor(&mut self, position: CursorPosition);

    /// Called on mode changes (future: support multiple video modes)
    fn set_video_mode(&mut self, mode: u8);

    /// Force a full redraw of the entire screen, ignoring cached state
    /// Used when the terminal state is known to be out of sync (e.g., after clearing screen)
    fn force_redraw(&mut self, buffer: &TextBuffer);

    /// Update graphics display (320x200, 4 colors)
    /// pixel_data: linear pixel array
    /// color_map: 4 VGA DAC indices from Attribute Controller registers [AC[0], AC[1], AC[2], AC[3]]
    /// Pixel value i maps to VGA DAC[color_map[i]] for final RGB color
    fn update_graphics_320x200(&mut self, pixel_data: &[u8], color_map: [u8; 4]) {
        // Default implementation: log warning
        let _ = (pixel_data, color_map);
        log::warn!("Graphics mode 320x200 not implemented for this platform");
    }

    /// Update graphics display (640x200, 2 colors)
    /// pixel_data: linear pixel array (1 bit per pixel), fg_color: foreground color
    /// composite: if true, render as CGA composite (160x200 16-color from nibble grouping)
    fn update_graphics_640x200(
        &mut self,
        pixel_data: &[u8],
        fg_color: u8,
        bg_color: u8,
        composite: bool,
    ) {
        let _ = (pixel_data, fg_color, bg_color, composite);
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
    fn update_display(&mut self, _buffer: &TextBuffer) {}
    fn update_cursor(&mut self, _position: CursorPosition) {}
    fn set_video_mode(&mut self, _mode: u8) {}
    fn force_redraw(&mut self, _buffer: &TextBuffer) {}
    // Graphics methods use default trait implementations
}

/// Core video state management
pub struct Video {
    /// Current cursor position
    cursor: CursorPosition,
    /// Text mode buffer (parsed representation)
    text_buffer: TextBuffer,
    /// Graphics mode buffer (optional, allocated when in graphics mode)
    cga_buffer: Option<CgaBuffer>,
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
    /// CGA composite mode: render 640x200 as composite artifact colors (160x200 16-color)
    /// Set when mode switches to 640x200 via port 0x3D8 (e.g., AGI games); cleared by INT 10h
    composite_mode: bool,
    /// Flag to sync graphics buffer from raw B800 memory on next update
    /// Set when transitioning from text → CGA graphics to preserve data (e.g., MS Flight Simulator)
    needs_memory_sync: bool,
    /// Video card type - limits which video modes are available
    card_type: VideoCardType,
    /// VGA Attribute Controller palette registers (16 entries)
    /// Maps pixel/attribute values (0-15) to VGA DAC indices (0-255)
    /// On CGA mode changes, AC[0-3] are synced from the CGA palette EGA indices
    /// Programs can reprogram these via port 0x3C0 for custom color mapping
    ac_palette: [u8; 16],
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
        Self::new_with_card_type(VideoCardType::default())
    }

    pub fn new_with_card_type(card_type: VideoCardType) -> Self {
        Self {
            cursor: CursorPosition::default(),
            text_buffer: TextBuffer::default(),
            cga_buffer: None,
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
            composite_mode: false,
            needs_memory_sync: false,
            card_type,
            ac_palette: Self::default_ac_palette(),
        }
    }

    /// Default AC palette: identity mapping (register i = i)
    fn default_ac_palette() -> [u8; 16] {
        let mut ac = [0u8; 16];
        for (i, entry) in ac.iter_mut().enumerate() {
            *entry = i as u8;
        }
        ac
    }

    /// Get the video card type
    pub fn card_type(&self) -> VideoCardType {
        self.card_type
    }

    /// Check if the given video mode is supported by the current video card type
    pub fn supports_mode(&self, mode: u8) -> bool {
        self.card_type.supports_mode(mode)
    }

    /// Read a single byte from video memory
    pub fn read_byte(&self, offset: usize) -> u8 {
        match &self.mode_type {
            VideoMode::Text { cols, .. } => self.text_buffer.read_byte(offset, *cols),
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200 => {
                // Graphics mode
                if let Some(ref buffer) = self.cga_buffer {
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
            VideoMode::Text { cols, .. } => self.text_buffer.write_byte(offset, *cols, value),
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200 => {
                // Graphics mode
                if let Some(ref mut buffer) = self.cga_buffer {
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
    pub fn get_buffer(&self) -> &TextBuffer {
        &self.text_buffer
    }

    /// Check if display needs updating
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark display as needing update
    pub fn set_dirty(&mut self) {
        self.dirty = true;
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
    /// Set video mode.
    /// `preserve_memory`: if true, sync raw B800 to graphics buffer on text→graphics transition
    /// (used for port-based mode switches where BIOS doesn't clear). INT 10h AH=00h passes false
    /// because real BIOS always clears video memory on mode change.
    pub fn set_mode(&mut self, mode: u8, preserve_memory: bool) {
        // Track previous mode type for sync decision
        let was_text_mode = matches!(self.mode_type, VideoMode::Text { .. });

        self.mode = mode;

        // Determine mode type and allocate appropriate buffer
        self.mode_type = match mode {
            0x00 | 0x01 => VideoMode::Text { cols: 40, rows: 25 },
            0x02 | 0x03 | 0x07 => VideoMode::Text { cols: 80, rows: 25 },
            0x04 | 0x05 => {
                self.cga_buffer = Some(CgaBuffer::new_320x200());
                VideoMode::Graphics320x200
            }
            0x06 => {
                self.cga_buffer = Some(CgaBuffer::new_640x200());
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

        // Set flag to sync from raw memory if transitioning from text to graphics
        let is_now_cga_graphics = matches!(
            self.mode_type,
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200
        );
        self.needs_memory_sync = was_text_mode && is_now_cga_graphics && preserve_memory;

        // Clear buffers on mode change
        if matches!(self.mode_type, VideoMode::Text { .. }) {
            self.text_buffer = TextBuffer::new();
            self.cga_buffer = None;
            self.ega_buffer = None;
        } else if matches!(self.mode_type, VideoMode::Graphics320x200x16) {
            self.cga_buffer = None;
        }

        // Reset VGA DAC palette and AC palette to defaults
        // This ensures programs that modify the palette don't leave the system
        // with invisible text (e.g., black text on black background)
        log::info!("VGA DAC: Resetting palette to defaults on mode change");
        self.vga_dac_palette = default_vga_palette();
        self.ac_palette = Self::default_ac_palette();

        // Reset CGA palette to defaults
        // For graphics modes (0x04-0x06), IBM CGA BIOS initializes to palette 1
        // For text modes, use palette 0
        self.palette = if matches!(
            self.mode_type,
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200
        ) {
            CgaPalette {
                background: 0,
                palette_id: 1, // Palette 1 (Cyan/Magenta/White) for graphics, high-intensity (VGA BIOS default)
                intensity: true,
            }
        } else {
            CgaPalette::new() // Palette 0 for text modes and EGA
        };

        // In CGA graphics modes, map AC palette registers to CGA colors.
        // This ensures pixel values 0-3 map to the correct CGA palette colors via VGA DAC.
        // Example: CGA palette 1 (high intensity) maps pixel 3 → VGA_DAC[15] (white),
        // not VGA_DAC[3] (cyan from default EGA palette).
        if matches!(
            self.mode_type,
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200
        ) {
            self.update_ac_from_cga_palette();
        }

        self.dirty = true;
        self.mode_changed = true; // Flag that controller needs notification
        log::info!("Video mode set to 0x{:02X} ({:?})", mode, self.mode_type);
    }

    /// Get current video mode
    pub fn get_mode(&self) -> u8 {
        self.mode
    }

    /// Set CGA composite mode flag
    pub fn set_composite_mode(&mut self, composite: bool) {
        self.composite_mode = composite;
    }

    /// Get CGA composite mode flag
    pub fn is_composite_mode(&self) -> bool {
        self.composite_mode
    }

    /// Sync graphics buffer from raw B800 memory (for text→graphics transitions)
    /// This is needed when programs write to B800 in text mode (e.g., as disk I/O buffer)
    /// then switch to graphics mode expecting that data to be visible.
    pub fn sync_from_raw_memory(&mut self, raw_memory: &[u8]) {
        if !self.needs_memory_sync {
            return;
        }

        match &self.mode_type {
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200 => {
                if let Some(ref mut buffer) = self.cga_buffer {
                    for (offset, &value) in raw_memory.iter().enumerate() {
                        if value != 0 {
                            // Only sync non-zero bytes to avoid clearing graphics
                            buffer.write_byte(offset, value);
                        }
                    }
                    log::debug!(
                        "Synced CGA graphics buffer from raw B800 memory ({} bytes)",
                        raw_memory.len()
                    );
                }
            }
            _ => {}
        }

        self.needs_memory_sync = false;
        self.dirty = true;
    }

    /// Check if memory sync is needed
    pub fn needs_memory_sync(&self) -> bool {
        self.needs_memory_sync
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

    /// Check if currently in CGA 4-color graphics mode (320x200 or 640x200)
    fn is_cga_graphics_mode(&self) -> bool {
        matches!(
            self.mode_type,
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200
        )
    }

    /// Scroll a text-cell window up in graphics mode.
    /// Dispatches to the appropriate buffer (CGA interlaced or EGA planar).
    /// `lines == 0` clears the entire window.
    pub fn scroll_up_window(&mut self, lines: u8, top: u8, left: u8, bottom: u8, right: u8) {
        let (lines, top, left, bottom, right) = (
            lines as usize,
            top as usize,
            left as usize,
            bottom as usize,
            right as usize,
        );
        match self.mode_type {
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200 => {
                if let Some(ref mut buf) = self.cga_buffer {
                    buf.scroll_up_window(lines, top, left, bottom, right);
                }
                self.dirty = true;
            }
            VideoMode::Graphics320x200x16 => {
                if let Some(ref mut buf) = self.ega_buffer {
                    buf.scroll_up_window(lines, top, left, bottom, right);
                }
                self.dirty = true;
            }
            VideoMode::Text { .. } => {}
        }
    }

    /// Scroll a text-cell window down in graphics mode.
    /// Dispatches to the appropriate buffer (CGA interlaced or EGA planar).
    /// `lines == 0` clears the entire window.
    pub fn scroll_down_window(&mut self, lines: u8, top: u8, left: u8, bottom: u8, right: u8) {
        let (lines, top, left, bottom, right) = (
            lines as usize,
            top as usize,
            left as usize,
            bottom as usize,
            right as usize,
        );
        match self.mode_type {
            VideoMode::Graphics320x200 | VideoMode::Graphics640x200 => {
                if let Some(ref mut buf) = self.cga_buffer {
                    buf.scroll_down_window(lines, top, left, bottom, right);
                }
                self.dirty = true;
            }
            VideoMode::Graphics320x200x16 => {
                if let Some(ref mut buf) = self.ega_buffer {
                    buf.scroll_down_window(lines, top, left, bottom, right);
                }
                self.dirty = true;
            }
            VideoMode::Text { .. } => {}
        }
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
        // Update AC palette registers to match CGA palette colors in graphics modes.
        if self.is_cga_graphics_mode() {
            self.update_ac_from_cga_palette();
        }
        self.dirty = true;
    }

    /// Get CGA palette register value
    pub fn get_palette_register(&self) -> u8 {
        self.palette.to_register()
    }

    /// Get graphics buffer (for rendering)
    pub fn get_graphics_buffer(&self) -> Option<&CgaBuffer> {
        self.cga_buffer.as_ref()
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
        if self.is_cga_graphics_mode() {
            self.update_ac_from_cga_palette();
        }
        self.dirty = true;
    }

    /// Set CGA intensity/bright mode
    pub fn set_cga_intensity(&mut self, enabled: bool) {
        self.palette.intensity = enabled;
        if self.is_cga_graphics_mode() {
            self.update_ac_from_cga_palette();
        }
        self.dirty = true;
    }

    /// Set CGA palette ID (0 or 1)
    pub fn set_cga_palette_id(&mut self, palette_id: u8) {
        self.palette.palette_id = palette_id & 0x01;
        if self.is_cga_graphics_mode() {
            self.update_ac_from_cga_palette();
        }
        self.dirty = true;
    }

    /// Update Attribute Controller palette registers 0-3 to match current CGA palette
    /// This maps CGA pixel values 0-3 to the correct VGA DAC indices via AC registers,
    /// matching real VGA hardware behavior where BIOS programs AC registers on CGA palette changes
    fn update_ac_from_cga_palette(&mut self) {
        let cga_colors = self.palette.get_colors(); // [bg, color1, color2, color3] as EGA indices

        // Set AC registers 0-3 to point to the EGA color indices
        // This way: pixel i → AC[i] → DAC[EGA_index] → RGB
        for (i, &ega_color) in cga_colors.iter().enumerate() {
            self.ac_palette[i] = ega_color;
        }

        log::debug!(
            "AC Palette: Synced registers 0-3 from CGA palette (id={}, intensity={}, bg={}) -> [{}, {}, {}, {}]",
            self.palette.palette_id,
            self.palette.intensity,
            self.palette.background,
            cga_colors[0],
            cga_colors[1],
            cga_colors[2],
            cga_colors[3]
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

    /// Get Attribute Controller palette registers
    /// Returns the 16-entry AC palette mapping pixel values to VGA DAC indices
    pub fn get_ac_palette(&self) -> &[u8; 16] {
        &self.ac_palette
    }

    /// Set an individual AC palette register
    /// index: register number (0-15), value: VGA DAC index (0-255)
    pub fn set_ac_register(&mut self, index: u8, value: u8) {
        let idx = (index & 0x0F) as usize;
        self.ac_palette[idx] = value;
        self.dirty = true;
        log::debug!("AC Palette: Register {} = {} (DAC index)", idx, value);
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
