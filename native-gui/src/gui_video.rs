use emu86_core::video::{
    CursorPosition, TEXT_MODE_COLS, TEXT_MODE_ROWS, TextCell, VideoController,
};
use pixels::Pixels;

use crate::font::{CHAR_HEIGHT, CHAR_WIDTH, Cp437Font};

/// Screen dimensions in pixels
#[allow(dead_code)]
pub const SCREEN_WIDTH: usize = TEXT_MODE_COLS * CHAR_WIDTH; // 640
#[allow(dead_code)]
pub const SCREEN_HEIGHT: usize = TEXT_MODE_ROWS * CHAR_HEIGHT; // 400

/// Cursor appearance constants
const CURSOR_START_ROW: usize = 14; // Cursor appears in bottom 2 rows of character
const CURSOR_END_ROW: usize = 16; // Exclusive

/// Convert VGA color (0-15) to RGB tuple
fn vga_to_rgb(vga_color: u8) -> [u8; 3] {
    match vga_color & 0x0F {
        0 => [0x00, 0x00, 0x00],  // Black
        1 => [0x00, 0x00, 0xAA],  // Blue
        2 => [0x00, 0xAA, 0x00],  // Green
        3 => [0x00, 0xAA, 0xAA],  // Cyan
        4 => [0xAA, 0x00, 0x00],  // Red
        5 => [0xAA, 0x00, 0xAA],  // Magenta
        6 => [0xAA, 0x55, 0x00],  // Brown
        7 => [0xAA, 0xAA, 0xAA],  // Light Gray
        8 => [0x55, 0x55, 0x55],  // Dark Gray
        9 => [0x55, 0x55, 0xFF],  // Light Blue
        10 => [0x55, 0xFF, 0x55], // Light Green
        11 => [0x55, 0xFF, 0xFF], // Light Cyan
        12 => [0xFF, 0x55, 0x55], // Light Red
        13 => [0xFF, 0x55, 0xFF], // Light Magenta
        14 => [0xFF, 0xFF, 0x55], // Yellow
        15 => [0xFF, 0xFF, 0xFF], // White
        _ => [0xFF, 0xFF, 0xFF],  // Fallback to white
    }
}

/// Video controller for GUI rendering
#[allow(dead_code)]
pub struct PixelsVideoController {
    font: Cp437Font,
    /// Current buffer state
    current_buffer: [TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS],
    /// Current cursor position
    current_cursor: Option<CursorPosition>,
    /// Cached buffer to track which cells have changed
    last_rendered_buffer: [TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS],
    /// Last rendered cursor position
    last_rendered_cursor: Option<CursorPosition>,
    /// Flag to force full redraw
    needs_full_redraw: bool,
}

#[allow(dead_code)]
impl PixelsVideoController {
    /// Create a new video controller
    pub fn new() -> Self {
        Self {
            font: Cp437Font::new(),
            current_buffer: [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS],
            current_cursor: None,
            last_rendered_buffer: [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS],
            last_rendered_cursor: None,
            needs_full_redraw: true,
        }
    }

    /// Render a single character cell at the given screen position
    fn render_cell(&self, frame: &mut [u8], row: usize, col: usize, cell: &TextCell) {
        let glyph = self.font.get_glyph(cell.character);
        let fg_color = vga_to_rgb(cell.attribute.foreground);
        let bg_color = vga_to_rgb(cell.attribute.background);

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

    /// Render the current state to a Pixels framebuffer
    /// This should be called from the main event loop after update_display/update_cursor
    pub fn render(&mut self, pixels: &mut Pixels) {
        let frame = pixels.frame_mut();

        if self.needs_full_redraw {
            // Clear screen and render everything
            frame.fill(0);
            for row in 0..TEXT_MODE_ROWS {
                for col in 0..TEXT_MODE_COLS {
                    let idx = row * TEXT_MODE_COLS + col;
                    self.render_cell(frame, row, col, &self.current_buffer[idx]);
                }
            }
            self.last_rendered_buffer
                .copy_from_slice(&self.current_buffer);
            self.needs_full_redraw = false;
        } else {
            // Only redraw changed cells for performance
            for row in 0..TEXT_MODE_ROWS {
                for col in 0..TEXT_MODE_COLS {
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
    }
}

impl Default for PixelsVideoController {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoController for PixelsVideoController {
    fn update_display(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {
        self.current_buffer.copy_from_slice(buffer);
    }

    fn update_cursor(&mut self, position: CursorPosition) {
        self.current_cursor = Some(position);
    }

    fn set_video_mode(&mut self, _mode: u8) {
        // Mark for full redraw
        self.needs_full_redraw = true;
        self.current_buffer = [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS];
        self.current_cursor = None;
    }

    fn force_redraw(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {
        self.current_buffer.copy_from_slice(buffer);
        self.needs_full_redraw = true;
    }
}
