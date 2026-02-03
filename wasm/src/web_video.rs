use emu86_core::font::{CHAR_HEIGHT, CHAR_WIDTH, Cp437Font};
use emu86_core::palette::TextModePalette;
use emu86_core::video::{
    CursorPosition, TEXT_MODE_COLS, TEXT_MODE_ROWS, TextCell, VideoController,
};
use wasm_bindgen::Clamped;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

const CANVAS_WIDTH: u32 = (TEXT_MODE_COLS * CHAR_WIDTH) as u32;
const CANVAS_HEIGHT: u32 = (TEXT_MODE_ROWS * CHAR_HEIGHT) as u32;

/// Web-based video controller using HTML5 Canvas with pixel-perfect rendering
pub struct WebVideo {
    context: CanvasRenderingContext2d,
    font: Cp437Font,
    /// Pixel buffer for the entire screen (RGBA format)
    buffer: Vec<u8>,
    /// Last rendered cursor position for cleanup
    last_cursor: Option<CursorPosition>,
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
            context,
            font: Cp437Font::new(),
            buffer,
            last_cursor: None,
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
        if cursor.row >= TEXT_MODE_ROWS || cursor.col >= TEXT_MODE_COLS {
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
        // Render all characters to the pixel buffer
        for row in 0..TEXT_MODE_ROWS {
            for col in 0..TEXT_MODE_COLS {
                let index = row * TEXT_MODE_COLS + col;
                self.render_char_to_buffer(row, col, &buffer[index]);
            }
        }
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
        // For now, only support mode 0x03 (80x25 text)
        if mode != 0x03 {
            log::warn!(
                "WASM: Video mode {} not yet implemented, only mode 0x03 (80x25 text) is supported",
                mode
            );
        }
    }

    fn force_redraw(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {
        // Same as update_display for web implementation
        self.update_display(buffer);
    }
}
