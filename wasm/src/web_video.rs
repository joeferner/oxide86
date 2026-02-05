use emu86_core::font::{CHAR_HEIGHT, CHAR_WIDTH, Cp437Font};
use emu86_core::palette::TextModePalette;
use emu86_core::video::{
    CgaPalette, CursorPosition, TEXT_MODE_COLS, TEXT_MODE_ROWS, TextCell, VideoController,
    VideoMode,
};
use wasm_bindgen::Clamped;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

const CANVAS_WIDTH: u32 = (TEXT_MODE_COLS * CHAR_WIDTH) as u32;
const CANVAS_HEIGHT: u32 = (TEXT_MODE_ROWS * CHAR_HEIGHT) as u32;

/// Web-based video controller using HTML5 Canvas with pixel-perfect rendering
pub struct WebVideo {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    font: Cp437Font,
    /// Pixel buffer for the entire screen (RGBA format)
    buffer: Vec<u8>,
    /// Last rendered cursor position for cleanup
    last_cursor: Option<CursorPosition>,
    /// Current video mode (for tracking text vs graphics)
    current_mode: VideoMode,
}

impl WebVideo {
    /// Create a new WebVideo controller
    ///
    /// # Arguments
    /// * `canvas` - The HTML canvas element to render to
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        // Set canvas size to 640x400 (80 chars * 8px × 25 rows * 16px)
        canvas.set_width(CANVAS_WIDTH);
        canvas.set_height(CANVAS_HEIGHT);

        let context = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("Failed to get 2d context"))?
            .dyn_into::<CanvasRenderingContext2d>()?;

        // Disable image smoothing for pixel-perfect rendering
        context.set_image_smoothing_enabled(false);

        let buffer_size = (CANVAS_WIDTH * CANVAS_HEIGHT * 4) as usize; // RGBA
        let buffer = vec![0u8; buffer_size];

        Ok(Self {
            canvas,
            context,
            font: Cp437Font::new(),
            buffer,
            last_cursor: None,
            current_mode: VideoMode::Text {
                cols: TEXT_MODE_COLS,
                rows: TEXT_MODE_ROWS,
            },
        })
    }

    /// Render a single character to the pixel buffer
    fn render_char_to_buffer(&mut self, row: usize, col: usize, cell: &TextCell) {
        let glyph = self.font.get_glyph(cell.character);
        let fg_color = TextModePalette::get_color(cell.attribute.foreground);
        let bg_color = TextModePalette::get_color(cell.attribute.background);

        let char_x = col * CHAR_WIDTH;
        let char_y = row * CHAR_HEIGHT;

        // Render each row of the glyph
        for (glyph_row, &glyph_byte) in glyph.iter().enumerate() {
            let pixel_y = char_y + glyph_row;

            // Render each pixel in the row (8 pixels from MSB to LSB)
            for bit in 0..8 {
                let pixel_x = char_x + bit;
                let is_fg = (glyph_byte & (0x80 >> bit)) != 0;
                let color = if is_fg { fg_color } else { bg_color };

                // Calculate buffer offset (RGBA format)
                let buffer_idx = ((pixel_y * CANVAS_WIDTH as usize) + pixel_x) * 4;

                self.buffer[buffer_idx] = color[0]; // R
                self.buffer[buffer_idx + 1] = color[1]; // G
                self.buffer[buffer_idx + 2] = color[2]; // B
                self.buffer[buffer_idx + 3] = 255; // A
            }
        }
    }

    /// Draw the cursor at the specified position
    fn draw_cursor(&mut self, cursor: &CursorPosition) {
        // Get actual mode dimensions
        let (cols, rows) = match self.current_mode {
            VideoMode::Text { cols, rows } => (cols, rows),
            _ => (TEXT_MODE_COLS, TEXT_MODE_ROWS),
        };

        if cursor.row >= rows || cursor.col >= cols {
            return;
        }

        let char_x = cursor.col * CHAR_WIDTH;
        let char_y = cursor.row * CHAR_HEIGHT;

        // Draw cursor as white underline on the last 2 rows of the character
        let cursor_color = [255u8, 255u8, 255u8]; // White

        for row_offset in (CHAR_HEIGHT - 2)..CHAR_HEIGHT {
            let pixel_y = char_y + row_offset;

            for col_offset in 0..CHAR_WIDTH {
                let pixel_x = char_x + col_offset;
                let buffer_idx = ((pixel_y * CANVAS_WIDTH as usize) + pixel_x) * 4;

                self.buffer[buffer_idx] = cursor_color[0]; // R
                self.buffer[buffer_idx + 1] = cursor_color[1]; // G
                self.buffer[buffer_idx + 2] = cursor_color[2]; // B
                self.buffer[buffer_idx + 3] = 255; // A
            }
        }
    }

    /// Update the canvas with the current buffer
    fn flush_to_canvas(&self) -> Result<(), JsValue> {
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&self.buffer),
            CANVAS_WIDTH,
            CANVAS_HEIGHT,
        )?;

        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        Ok(())
    }

    /// Render the entire screen
    fn render_full_screen(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {
        // Get actual mode dimensions
        let (actual_cols, actual_rows) = match self.current_mode {
            VideoMode::Text { cols, rows } => (cols, rows),
            _ => (TEXT_MODE_COLS, TEXT_MODE_ROWS),
        };

        // Render all characters to the pixel buffer
        for row in 0..actual_rows {
            for col in 0..actual_cols {
                let index = row * TEXT_MODE_COLS + col;
                self.render_char_to_buffer(row, col, &buffer[index]);
            }
        }
    }

    /// Render graphics mode 320x200 (4-color) using ImageData API
    fn render_graphics_320x200(
        &mut self,
        pixel_data: &[u8],
        palette: &CgaPalette,
    ) -> Result<(), JsValue> {
        // Resize canvas for graphics mode
        self.canvas.set_width(640); // 320 * 2 (scaled)
        self.canvas.set_height(400); // 200 * 2 (scaled)

        let colors = palette.get_colors();
        let width = 320;
        let height = 200;
        let scale = 2;

        // Create ImageData buffer for scaled output
        let scaled_width = width * scale;
        let scaled_height = height * scale;
        let mut image_data_buf = vec![0u8; scaled_width * scaled_height * 4]; // RGBA

        // Iterate through all CGA pixels
        for y in 0..height {
            for x in 0..width {
                // Extract pixel color (2 bits per pixel, 4 pixels per byte)
                let byte_offset = y * 80 + x / 4;
                let pixel_in_byte = x % 4;
                let byte_val = pixel_data[byte_offset];
                let shift = 6 - (pixel_in_byte * 2);
                let color_index = ((byte_val >> shift) & 0x03) as usize;

                // Get RGB color from palette
                let vga_color = colors[color_index];
                let rgb = TextModePalette::get_color(vga_color);

                // Draw scaled pixel (2x2 screen pixels per CGA pixel)
                for dy in 0..scale {
                    for dx in 0..scale {
                        let screen_x = x * scale + dx;
                        let screen_y = y * scale + dy;
                        let pixel_offset = (screen_y * scaled_width + screen_x) * 4;

                        image_data_buf[pixel_offset] = rgb[0]; // R
                        image_data_buf[pixel_offset + 1] = rgb[1]; // G
                        image_data_buf[pixel_offset + 2] = rgb[2]; // B
                        image_data_buf[pixel_offset + 3] = 255; // A
                    }
                }
            }
        }

        // Create ImageData and render to canvas
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&image_data_buf),
            scaled_width as u32,
            scaled_height as u32,
        )?;

        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        Ok(())
    }

    /// Render graphics mode 640x200 (2-color) using ImageData API
    fn render_graphics_640x200(
        &mut self,
        pixel_data: &[u8],
        fg_color: u8,
        bg_color: u8,
    ) -> Result<(), JsValue> {
        // Resize canvas for graphics mode
        self.canvas.set_width(640); // 640 pixels (no horizontal scaling)
        self.canvas.set_height(400); // 200 * 2 (scaled vertically)

        let fg_rgb = TextModePalette::get_color(fg_color);
        let bg_rgb = TextModePalette::get_color(bg_color);

        let width = 640;
        let height = 200;
        let scale = 2; // 2x vertical scaling only

        let scaled_height = height * scale;
        let mut image_data_buf = vec![0u8; width * scaled_height * 4]; // RGBA

        for y in 0..height {
            for x in 0..width {
                // Extract pixel (1 bit per pixel, 8 pixels per byte)
                let byte_offset = y * 80 + x / 8;
                let pixel_in_byte = x % 8;
                let byte_val = pixel_data[byte_offset];
                let bit_mask = 0x80 >> pixel_in_byte;
                let is_set = (byte_val & bit_mask) != 0;

                let rgb = if is_set { fg_rgb } else { bg_rgb };

                // Draw scaled pixel (1x width, 2x height for 640x400)
                for dy in 0..scale {
                    let screen_x = x;
                    let screen_y = y * scale + dy;
                    let pixel_offset = (screen_y * width + screen_x) * 4;

                    image_data_buf[pixel_offset] = rgb[0]; // R
                    image_data_buf[pixel_offset + 1] = rgb[1]; // G
                    image_data_buf[pixel_offset + 2] = rgb[2]; // B
                    image_data_buf[pixel_offset + 3] = 255; // A
                }
            }
        }

        // Create ImageData and render to canvas
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&image_data_buf),
            width as u32,
            scaled_height as u32,
        )?;

        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        Ok(())
    }
}

impl VideoController for WebVideo {
    fn update_display(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {
        // Render all characters
        self.render_full_screen(buffer);

        // Flush to canvas
        if let Err(e) = self.flush_to_canvas() {
            log::error!("Failed to update display: {:?}", e);
        }
    }

    fn update_cursor(&mut self, cursor: CursorPosition) {
        // We need to redraw the character at the old cursor position to erase it,
        // then draw the new cursor. However, we don't have the buffer here.
        // For now, just store the cursor position and draw it when we update the display.
        // A more efficient implementation would store the buffer and redraw only affected cells.
        self.last_cursor = Some(cursor);

        // Draw cursor on current buffer
        self.draw_cursor(&cursor);

        // Flush to canvas
        if let Err(e) = self.flush_to_canvas() {
            log::error!("Failed to update cursor: {:?}", e);
        }
    }

    fn set_video_mode(&mut self, mode: u8) {
        // Update current mode based on video mode number
        self.current_mode = match mode {
            0x00 | 0x01 => VideoMode::Text { cols: 40, rows: 25 },
            0x02 | 0x03 | 0x07 => VideoMode::Text { cols: 80, rows: 25 },
            0x04 | 0x05 => VideoMode::Graphics320x200,
            0x06 => VideoMode::Graphics640x200,
            _ => {
                log::warn!(
                    "WASM: Unsupported video mode 0x{:02X}, defaulting to text",
                    mode
                );
                VideoMode::Text { cols: 80, rows: 25 }
            }
        };

        // Resize canvas based on mode
        match self.current_mode {
            VideoMode::Text { .. } => {
                self.canvas.set_width(CANVAS_WIDTH);
                self.canvas.set_height(CANVAS_HEIGHT);
                let buffer_size = (CANVAS_WIDTH * CANVAS_HEIGHT * 4) as usize;
                self.buffer.resize(buffer_size, 0);
            }
            VideoMode::Graphics320x200 => {
                // Canvas will be resized in render_graphics_320x200
                log::info!("WASM: Switched to 320x200 graphics mode");
            }
            VideoMode::Graphics640x200 => {
                // Canvas will be resized in render_graphics_640x200
                log::info!("WASM: Switched to 640x200 graphics mode");
            }
        }
    }

    fn force_redraw(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {
        // Same as update_display for web implementation
        self.update_display(buffer);
    }

    fn update_graphics_320x200(&mut self, pixel_data: &[u8], palette: &CgaPalette) {
        if let Err(e) = self.render_graphics_320x200(pixel_data, palette) {
            log::error!("Failed to render 320x200 graphics: {:?}", e);
        }
    }

    fn update_graphics_640x200(&mut self, pixel_data: &[u8], fg_color: u8, bg_color: u8) {
        if let Err(e) = self.render_graphics_640x200(pixel_data, fg_color, bg_color) {
            log::error!("Failed to render 640x200 graphics: {:?}", e);
        }
    }
}
