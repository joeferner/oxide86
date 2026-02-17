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
    /// CGA color map for 320x200 mode (4 VGA DAC indices from AC registers)
    graphics_color_map: Option<[u8; 4]>,
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
            graphics_color_map: None,
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

    /// Render a single character cell at the given screen position
    fn render_cell(&self, frame: &mut [u8], row: usize, col: usize, cell: &TextCell) {
        emu86_core::video::render::render_text_cell(
            &self.font,
            row,
            col,
            cell,
            &self.vga_dac_palette,
            SCREEN_WIDTH,
            frame,
        );
    }

    /// Render cursor at the given position
    fn render_cursor_at(&self, frame: &mut [u8], position: CursorPosition) {
        emu86_core::video::render::render_cursor(&position, SCREEN_WIDTH, frame);
    }

    /// Render graphics mode 320x200 (4-color) to framebuffer
    fn render_graphics_320x200(&self, frame: &mut [u8]) {
        if let (Some(pixel_data), Some(color_map)) = (&self.graphics_data, &self.graphics_color_map)
        {
            emu86_core::video::render::render_cga_320x200(
                pixel_data,
                color_map,
                &self.vga_dac_palette,
                frame,
            );
        }
    }

    /// Render graphics mode 640x200 to framebuffer.
    /// In composite mode: when in mode 0x04 (2bpp), renders each 2-bit pixel individually
    /// using composite color palette (320x200 scaled 2x2 to 640x400).
    /// In RGB mode: standard per-pixel B&W rendering (640x200).
    fn render_graphics_640x200(&self, frame: &mut [u8]) {
        if let Some(pixel_data) = &self.graphics_data {
            if self.graphics_composite {
                emu86_core::video::composite::render_composite_2bpp(pixel_data, frame);
            } else {
                emu86_core::video::render::render_cga_640x200_bw(
                    pixel_data,
                    self.graphics_fg_color,
                    self.graphics_bg_color,
                    frame,
                );
            }
        }
    }

    /// Render EGA graphics mode 320x200 (16-color) to framebuffer
    fn render_graphics_320x200x16(&self, frame: &mut [u8]) {
        if let Some(pixel_data) = &self.graphics_data {
            emu86_core::video::render::render_ega_320x200x16(
                pixel_data,
                &self.vga_dac_palette,
                frame,
            );
        }
    }

    /// Render VGA graphics mode 320x200 (256-color, mode 13h) to framebuffer
    fn render_graphics_320x200x256(&self, frame: &mut [u8]) {
        if let Some(pixel_data) = &self.graphics_data {
            emu86_core::video::render::render_vga_320x200x256(
                pixel_data,
                &self.vga_dac_palette,
                frame,
            );
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
            VideoMode::Graphics320x200x256 => {
                self.render_graphics_320x200x256(&mut buffer);
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
            VideoMode::Graphics320x200x256 => {
                self.render_graphics_320x200x256(frame);
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
            graphics_color_map: None,
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
            0x13 => VideoMode::Graphics320x200x256,
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

    fn update_graphics_320x200(&mut self, pixel_data: &[u8], color_map: [u8; 4]) {
        // Store pixel data and AC color map (VGA DAC indices)
        self.graphics_data = Some(pixel_data.to_vec());
        self.graphics_color_map = Some(color_map);
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

    fn update_graphics_320x200x256(&mut self, pixel_data: &[u8]) {
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
