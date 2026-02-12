use emu86_core::video::VideoMode;
use emu86_core::video::text::{TextBuffer, TextCell};
use emu86_core::{
    Cp437Font, TextModePalette,
    font::{CHAR_HEIGHT, CHAR_WIDTH},
    video::{CursorPosition, TEXT_MODE_COLS, TEXT_MODE_ROWS, VideoController},
};
use pixels::Pixels;

/// Screen dimensions in pixels
#[allow(dead_code)]
pub const SCREEN_WIDTH: usize = TEXT_MODE_COLS * CHAR_WIDTH; // 640
#[allow(dead_code)]
pub const SCREEN_HEIGHT: usize = TEXT_MODE_ROWS * CHAR_HEIGHT; // 400

/// Cursor appearance constants
const CURSOR_START_ROW: usize = 14; // Cursor appears in bottom 2 rows of character
const CURSOR_END_ROW: usize = 16; // Exclusive

/// Video controller for GUI rendering
#[allow(dead_code)]
pub struct PixelsVideoController {
    font: Cp437Font,
    /// Current buffer state
    current_buffer: TextBuffer,
    /// Current cursor position
    current_cursor: Option<CursorPosition>,
    /// Cached buffer to track which cells have changed
    last_rendered_buffer: TextBuffer,
    /// Last rendered cursor position
    last_rendered_cursor: Option<CursorPosition>,
    /// Flag to force full redraw
    needs_full_redraw: bool,
    /// Flag to indicate if there are pending updates since last render
    has_pending_updates: bool,
    /// Current video mode (for tracking text vs graphics)
    current_mode: VideoMode,
    /// Graphics mode pixel data (for 320x200 or 640x200 modes)
    graphics_data: Option<Vec<u8>>,
    /// Graphics mode colors (for 640x200 2-color mode)
    graphics_fg_color: u8,
    graphics_bg_color: u8,
    /// CGA palette for 320x200 mode (4 EGA color indices 0-15)
    graphics_palette: Option<[u8; 4]>,
    /// VGA DAC palette (256 RGB triplets, 6-bit per component 0-63)
    vga_dac_palette: [[u8; 3]; 256],
    /// CGA composite rendering mode for 640x200
    graphics_composite: bool,
}

#[allow(dead_code)]
impl PixelsVideoController {
    /// Create a new video controller
    pub fn new() -> Self {
        Self {
            font: Cp437Font::new(),
            current_buffer: TextBuffer::default(),
            current_cursor: None,
            last_rendered_buffer: TextBuffer::default(),
            last_rendered_cursor: None,
            needs_full_redraw: true,
            has_pending_updates: true,
            current_mode: VideoMode::Text {
                cols: TEXT_MODE_COLS,
                rows: TEXT_MODE_ROWS,
            },
            graphics_data: None,
            graphics_fg_color: 15, // White
            graphics_bg_color: 0,  // Black
            graphics_palette: None,
            vga_dac_palette: Self::default_vga_dac_palette(),
            graphics_composite: false,
        }
    }

    /// Create default VGA DAC palette (same as core::video::default_vga_palette)
    fn default_vga_dac_palette() -> [[u8; 3]; 256] {
        let mut palette = [[0u8; 3]; 256];
        // Initialize first 16 colors with EGA defaults (6-bit RGB values 0-63)
        for (i, entry) in palette.iter_mut().enumerate().take(16) {
            *entry = TextModePalette::get_dac_color(i as u8);
        }
        palette
    }

    /// Check if there are pending updates that need rendering
    pub fn has_pending_updates(&self) -> bool {
        self.has_pending_updates
    }

    /// Convert 6-bit VGA DAC RGB (0-63) to 8-bit RGB (0-255)
    /// Uses the standard VGA conversion: value * 255 / 63
    fn dac_to_rgb(&self, dac_value: u8) -> u8 {
        // Standard VGA DAC conversion: multiply by ~4.047619
        // Using ((value << 2) | (value >> 4)) for accuracy
        let val = dac_value & 0x3F; // Ensure 6-bit
        (val << 2) | (val >> 4)
    }

    /// Get 8-bit RGB color from VGA DAC palette
    fn get_palette_color(&self, color_index: u8) -> [u8; 3] {
        let dac_color = self.vga_dac_palette[color_index as usize];
        [
            self.dac_to_rgb(dac_color[0]),
            self.dac_to_rgb(dac_color[1]),
            self.dac_to_rgb(dac_color[2]),
        ]
    }

    /// Render a single character cell at the given screen position
    fn render_cell(&self, frame: &mut [u8], row: usize, col: usize, cell: &TextCell) {
        let glyph = self.font.get_glyph(cell.character);
        let fg_color = self.get_palette_color(cell.attribute.foreground);
        let bg_color = self.get_palette_color(cell.attribute.background);

        let start_x = col * CHAR_WIDTH;
        let start_y = row * CHAR_HEIGHT;

        // Render each row of the glyph
        for (glyph_row, &glyph_byte) in glyph.iter().enumerate() {
            let pixel_y = start_y + glyph_row;

            // Render each pixel in the row
            for bit_pos in 0..8 {
                let pixel_x = start_x + bit_pos;
                let is_foreground = (glyph_byte & (0x80 >> bit_pos)) != 0;

                let color = if is_foreground { fg_color } else { bg_color };

                // Calculate frame buffer offset (RGBA format)
                let offset = (pixel_y * SCREEN_WIDTH + pixel_x) * 4;

                frame[offset] = color[0]; // R
                frame[offset + 1] = color[1]; // G
                frame[offset + 2] = color[2]; // B
                frame[offset + 3] = 0xFF; // A (always opaque)
            }
        }
    }

    /// Render cursor at the given position
    fn render_cursor_at(&self, frame: &mut [u8], position: CursorPosition) {
        let start_x = position.col * CHAR_WIDTH;
        let start_y = position.row * CHAR_HEIGHT;

        // Cursor is a white block in the bottom 2 rows
        for cursor_row in CURSOR_START_ROW..CURSOR_END_ROW {
            let pixel_y = start_y + cursor_row;

            for bit_pos in 0..8 {
                let pixel_x = start_x + bit_pos;
                let offset = (pixel_y * SCREEN_WIDTH + pixel_x) * 4;

                frame[offset] = 0xFF; // R
                frame[offset + 1] = 0xFF; // G
                frame[offset + 2] = 0xFF; // B
                frame[offset + 3] = 0xFF; // A
            }
        }
    }

    /// Render graphics mode 320x200 (4-color) to framebuffer
    fn render_graphics_320x200(&self, frame: &mut [u8]) {
        if let (Some(pixel_data), Some(cga_palette)) = (&self.graphics_data, &self.graphics_palette)
        {
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

                    // Map pixel value to CGA palette entry (EGA color index)
                    // For CGA compatibility, use fixed CGA palette colors, not VGA DAC
                    let ega_color = cga_palette[color_index];
                    let rgb = TextModePalette::get_color(ega_color);

                    // Draw scaled pixel (2x2 screen pixels per CGA pixel)
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let screen_x = x * scale + dx;
                            let screen_y = y * scale + dy;
                            let offset = (screen_y * SCREEN_WIDTH + screen_x) * 4;

                            frame[offset] = rgb[0]; // R
                            frame[offset + 1] = rgb[1]; // G
                            frame[offset + 2] = rgb[2]; // B
                            frame[offset + 3] = 0xFF; // A (opaque)
                        }
                    }
                }
            }
        }
    }

    /// Render graphics mode 640x200 to framebuffer.
    /// In composite mode: when in mode 0x04 (2bpp), renders each 2-bit pixel individually
    /// using composite color palette (320x200 scaled 2x2 to 640x400).
    /// In RGB mode: standard per-pixel B&W rendering (640x200).
    fn render_graphics_640x200(&self, frame: &mut [u8]) {
        if let Some(pixel_data) = &self.graphics_data {
            if self.graphics_composite {
                // Use shared composite rendering logic from core
                emu86_core::video::composite::render_composite_2bpp(pixel_data, frame);
            } else {
                // RGB mode: per-pixel B&W, 640x200 scaled 1x2 to 640x400
                let fg_rgb = TextModePalette::get_color(self.graphics_fg_color);
                let bg_rgb = TextModePalette::get_color(self.graphics_bg_color);
                for y in 0..200 {
                    for x in 0..640 {
                        let byte_val = pixel_data[y * 80 + x / 8];
                        let bit_mask = 0x80 >> (x % 8);
                        let rgb = if (byte_val & bit_mask) != 0 {
                            fg_rgb
                        } else {
                            bg_rgb
                        };
                        for dy in 0..2 {
                            let screen_y = y * 2 + dy;
                            let offset = (screen_y * SCREEN_WIDTH + x) * 4;
                            frame[offset] = rgb[0];
                            frame[offset + 1] = rgb[1];
                            frame[offset + 2] = rgb[2];
                            frame[offset + 3] = 0xFF;
                        }
                    }
                }
            }
        }
    }

    /// Render EGA graphics mode 320x200 (16-color) to framebuffer
    fn render_graphics_320x200x16(&self, frame: &mut [u8]) {
        if let Some(pixel_data) = &self.graphics_data {
            let scale = 2; // Scale factor: 320x200 -> 640x400 window

            for y in 0..200 {
                for x in 0..320 {
                    // Each byte in pixel_data is a 0-15 color index
                    let color_index = pixel_data[y * 320 + x] as usize;

                    // Look up RGB from VGA DAC palette (6-bit values, scale to 8-bit)
                    let dac = self.vga_dac_palette[color_index];
                    let r = (dac[0] << 2) | (dac[0] >> 4);
                    let g = (dac[1] << 2) | (dac[1] >> 4);
                    let b = (dac[2] << 2) | (dac[2] >> 4);

                    // Draw scaled pixel (2x2 screen pixels per EGA pixel)
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let screen_x = x * scale + dx;
                            let screen_y = y * scale + dy;
                            let offset = (screen_y * SCREEN_WIDTH + screen_x) * 4;
                            frame[offset] = r;
                            frame[offset + 1] = g;
                            frame[offset + 2] = b;
                            frame[offset + 3] = 0xFF;
                        }
                    }
                }
            }
        }
    }

    /// Render current state to a raw RGBA buffer (for screenshots)
    /// Returns a buffer of size SCREEN_WIDTH * SCREEN_HEIGHT * 4 bytes (RGBA)
    pub fn render_to_buffer(&self) -> Vec<u8> {
        let mut buffer = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4];

        // Check if we're in graphics mode
        match self.current_mode {
            VideoMode::Graphics320x200 => {
                if self.graphics_composite {
                    self.render_graphics_640x200(&mut buffer);
                } else {
                    self.render_graphics_320x200(&mut buffer);
                }
                return buffer;
            }
            VideoMode::Graphics640x200 => {
                self.render_graphics_640x200(&mut buffer);
                return buffer;
            }
            VideoMode::Graphics320x200x16 => {
                self.render_graphics_320x200x16(&mut buffer);
                return buffer;
            }
            VideoMode::Text { .. } => {
                // Continue with text mode rendering below
            }
        }

        // Get actual mode dimensions
        let (actual_cols, actual_rows) = match self.current_mode {
            VideoMode::Text { cols, rows } => (cols, rows),
            _ => (TEXT_MODE_COLS, TEXT_MODE_ROWS),
        };

        // Render all cells
        for row in 0..actual_rows {
            for col in 0..actual_cols {
                let idx = row * TEXT_MODE_COLS + col;
                self.render_cell(&mut buffer, row, col, &self.current_buffer[idx]);
            }
        }

        // Render cursor if visible
        if let Some(pos) = self.current_cursor {
            self.render_cursor_at(&mut buffer, pos);
        }

        buffer
    }

    /// Render the current state to a Pixels framebuffer
    /// This should be called from the main event loop after update_display/update_cursor
    pub fn render(&mut self, pixels: &mut Pixels) {
        let frame = pixels.frame_mut();

        // Check if we're in graphics mode
        match self.current_mode {
            VideoMode::Graphics320x200 => {
                if self.graphics_composite {
                    self.render_graphics_640x200(frame);
                } else {
                    self.render_graphics_320x200(frame);
                }
                self.has_pending_updates = false;
                return;
            }
            VideoMode::Graphics640x200 => {
                self.render_graphics_640x200(frame);
                self.has_pending_updates = false;
                return;
            }
            VideoMode::Graphics320x200x16 => {
                self.render_graphics_320x200x16(frame);
                self.has_pending_updates = false;
                return;
            }
            VideoMode::Text { .. } => {
                // Continue with text mode rendering below
            }
        }

        // Get actual mode dimensions
        let (actual_cols, actual_rows) = match self.current_mode {
            VideoMode::Text { cols, rows } => (cols, rows),
            _ => (TEXT_MODE_COLS, TEXT_MODE_ROWS),
        };

        if self.needs_full_redraw {
            // Clear screen and render everything
            frame.fill(0);
            for row in 0..actual_rows {
                for col in 0..actual_cols {
                    let idx = row * TEXT_MODE_COLS + col;
                    self.render_cell(frame, row, col, &self.current_buffer[idx]);
                }
            }
            self.last_rendered_buffer.copy_from(&self.current_buffer);
            self.needs_full_redraw = false;
        } else {
            // Only redraw changed cells for performance
            for row in 0..actual_rows {
                for col in 0..actual_cols {
                    let idx = row * TEXT_MODE_COLS + col;
                    if self.current_buffer[idx] != self.last_rendered_buffer[idx] {
                        self.render_cell(frame, row, col, &self.current_buffer[idx]);
                        self.last_rendered_buffer[idx] = self.current_buffer[idx];
                    }
                }
            }
        }

        // Render cursor
        if self.current_cursor != self.last_rendered_cursor {
            // Clear old cursor by redrawing the cell
            if let Some(old_pos) = self.last_rendered_cursor {
                let idx = old_pos.row * TEXT_MODE_COLS + old_pos.col;
                if idx < self.current_buffer.len() {
                    self.render_cell(frame, old_pos.row, old_pos.col, &self.current_buffer[idx]);
                }
            }

            // Draw new cursor
            if let Some(new_pos) = self.current_cursor {
                self.render_cursor_at(frame, new_pos);
            }

            self.last_rendered_cursor = self.current_cursor;
        }

        // Clear the pending updates flag after rendering
        self.has_pending_updates = false;
    }
}

impl Default for PixelsVideoController {
    fn default() -> Self {
        Self {
            font: Cp437Font::new(),
            current_buffer: TextBuffer::new(),
            current_cursor: None,
            last_rendered_buffer: TextBuffer::new(),
            last_rendered_cursor: None,
            needs_full_redraw: true,
            has_pending_updates: true,
            current_mode: VideoMode::Text {
                cols: TEXT_MODE_COLS,
                rows: TEXT_MODE_ROWS,
            },
            graphics_data: None,
            graphics_fg_color: 15, // White
            graphics_bg_color: 0,  // Black
            graphics_palette: None,
            vga_dac_palette: Self::default_vga_dac_palette(),
            graphics_composite: false,
        }
    }
}

impl VideoController for PixelsVideoController {
    fn update_display(&mut self, buffer: &TextBuffer) {
        self.current_buffer.copy_from(buffer);
        self.has_pending_updates = true;
    }

    fn update_cursor(&mut self, position: CursorPosition) {
        // Only mark as pending if cursor actually moved
        if self.current_cursor != Some(position) {
            self.current_cursor = Some(position);
            self.has_pending_updates = true;
        }
    }

    fn set_video_mode(&mut self, mode: u8) {
        // Update current mode based on video mode number
        self.current_mode = match mode {
            0x00 | 0x01 => VideoMode::Text { cols: 40, rows: 25 },
            0x02 | 0x03 | 0x07 => VideoMode::Text { cols: 80, rows: 25 },
            0x04 | 0x05 => VideoMode::Graphics320x200,
            0x06 => VideoMode::Graphics640x200,
            0x0D => VideoMode::Graphics320x200x16,
            _ => VideoMode::Text { cols: 80, rows: 25 }, // Default to text mode
        };

        // Clear graphics data when switching modes
        self.graphics_data = None;

        // Mark for full redraw
        self.needs_full_redraw = true;
        self.has_pending_updates = true;
        self.current_buffer = TextBuffer::new();
        self.current_cursor = None;
    }

    fn force_redraw(&mut self, buffer: &TextBuffer) {
        self.current_buffer.copy_from(buffer);
        self.needs_full_redraw = true;
        self.has_pending_updates = true;
    }

    fn update_graphics_320x200(&mut self, pixel_data: &[u8], cga_palette: [u8; 4]) {
        // Store pixel data and CGA palette (EGA color indices)
        self.graphics_data = Some(pixel_data.to_vec());
        self.graphics_palette = Some(cga_palette);
        self.graphics_composite = false; // Disable composite mode for normal 320x200

        // Mark for update
        self.has_pending_updates = true;
    }

    fn update_graphics_640x200(
        &mut self,
        pixel_data: &[u8],
        fg_color: u8,
        bg_color: u8,
        composite: bool,
    ) {
        self.graphics_data = Some(pixel_data.to_vec());
        self.graphics_fg_color = fg_color;
        self.graphics_bg_color = bg_color;
        self.graphics_composite = composite;
        self.has_pending_updates = true;
    }

    fn update_graphics_320x200x16(&mut self, pixel_data: &[u8]) {
        self.graphics_data = Some(pixel_data.to_vec());
        self.has_pending_updates = true;
    }

    fn update_vga_dac_palette(&mut self, palette: &[[u8; 3]; 256]) {
        // Update stored VGA DAC palette
        self.vga_dac_palette.copy_from_slice(palette);

        // Log first 16 colors (standard text mode colors)
        log::trace!("Renderer: Updating VGA DAC palette");
        log::trace!(
            "  Color 0 (Black):      RGB({:2}, {:2}, {:2}) 6-bit",
            palette[0][0],
            palette[0][1],
            palette[0][2]
        );
        log::trace!(
            "  Color 7 (Light Gray): RGB({:2}, {:2}, {:2}) 6-bit",
            palette[7][0],
            palette[7][1],
            palette[7][2]
        );
        log::trace!(
            "  Color 15 (White):     RGB({:2}, {:2}, {:2}) 6-bit",
            palette[15][0],
            palette[15][1],
            palette[15][2]
        );

        // Mark for full redraw since colors changed
        self.needs_full_redraw = true;
        self.has_pending_updates = true;
    }
}
