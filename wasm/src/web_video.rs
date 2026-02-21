use oxide86_core::font::{CHAR_HEIGHT, CHAR_WIDTH, Cp437Font};
use oxide86_core::palette::TextModePalette;
use oxide86_core::video::text::{TextBuffer, TextCell};
use oxide86_core::video::{
    CursorPosition, TEXT_MODE_COLS, TEXT_MODE_ROWS, VideoController, VideoMode,
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
    /// CGA color map for 320x200 mode (4 VGA DAC indices from AC registers)
    graphics_color_map: Option<[u8; 4]>,
    /// VGA DAC palette (256 colors, RGB 6-bit values 0-63)
    vga_dac_palette: [[u8; 3]; 256],
    /// BIOS logo overlay: RGBA pixels + (width, height).
    /// Blended into the pixel buffer before every canvas flush until cleared.
    logo_overlay: Option<(Vec<u8>, usize, usize)>,
}

impl WebVideo {
    /// Initialize VGA DAC palette with EGA defaults
    fn default_vga_dac_palette() -> [[u8; 3]; 256] {
        let mut palette = [[0u8; 3]; 256];
        // Initialize first 16 colors with EGA defaults (6-bit RGB values 0-63)
        for (i, entry) in palette.iter_mut().enumerate().take(16) {
            *entry = TextModePalette::get_dac_color(i as u8);
        }
        palette
    }

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
            graphics_color_map: None,
            vga_dac_palette: Self::default_vga_dac_palette(),
            logo_overlay: None,
        })
    }

    /// Render a single character to the pixel buffer
    fn render_char_to_buffer(&mut self, row: usize, col: usize, cell: &TextCell) {
        oxide86_core::video::render::render_text_cell(
            &self.font,
            row,
            col,
            cell,
            &self.vga_dac_palette,
            CANVAS_WIDTH as usize,
            &mut self.buffer,
        );
    }

    /// Draw the cursor at the specified position
    fn draw_cursor(&mut self, cursor: &CursorPosition) {
        let (cols, rows) = match self.current_mode {
            VideoMode::Text { cols, rows } => (cols, rows),
            _ => (TEXT_MODE_COLS, TEXT_MODE_ROWS),
        };

        if cursor.row >= rows || cursor.col >= cols {
            return;
        }

        oxide86_core::video::render::render_cursor(cursor, CANVAS_WIDTH as usize, &mut self.buffer);
    }

    /// Blit the stored logo overlay into the pixel buffer (no-op if no overlay).
    /// Must be called after text/graphics rendering and before `flush_to_canvas`.
    fn blit_logo_overlay(&mut self) {
        if let Some((pixels, ow, oh)) = &self.logo_overlay {
            let frame_stride = CANVAS_WIDTH as usize * 4;
            for y in 0..*oh {
                let src = &pixels[y * ow * 4..(y + 1) * ow * 4];
                let dst_start = y * frame_stride;
                self.buffer[dst_start..dst_start + ow * 4].copy_from_slice(src);
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
    fn render_full_screen(&mut self, buffer: &TextBuffer) {
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
    fn render_graphics_320x200(&mut self, pixel_data: &[u8]) -> Result<(), JsValue> {
        let color_map = match &self.graphics_color_map {
            Some(p) => *p,
            None => return Ok(()),
        };

        self.canvas.set_width(640);
        self.canvas.set_height(400);

        let mut image_data_buf = vec![0u8; 640 * 400 * 4];
        oxide86_core::video::render::render_cga_320x200(
            pixel_data,
            &color_map,
            &self.vga_dac_palette,
            &mut image_data_buf,
        );

        let image_data =
            ImageData::new_with_u8_clamped_array_and_sh(Clamped(&image_data_buf), 640, 400)?;
        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        Ok(())
    }

    /// Render graphics mode 640x200 to canvas.
    fn render_graphics_640x200(
        &mut self,
        pixel_data: &[u8],
        fg_color: u8,
        bg_color: u8,
        composite: bool,
    ) -> Result<(), JsValue> {
        self.canvas.set_width(640);
        self.canvas.set_height(400);

        let mut image_data_buf = vec![0u8; 640 * 400 * 4];

        if composite {
            oxide86_core::video::composite::render_composite_2bpp(pixel_data, &mut image_data_buf);
        } else {
            oxide86_core::video::render::render_cga_640x200_bw(
                pixel_data,
                fg_color,
                bg_color,
                &mut image_data_buf,
            );
        }

        let image_data =
            ImageData::new_with_u8_clamped_array_and_sh(Clamped(&image_data_buf), 640, 400)?;
        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        Ok(())
    }

    /// Render EGA graphics mode 320x200 (16-color) using ImageData API
    fn render_graphics_320x200x16(&mut self, pixel_data: &[u8]) -> Result<(), JsValue> {
        self.canvas.set_width(640);
        self.canvas.set_height(400);

        let mut image_data_buf = vec![0u8; 640 * 400 * 4];
        oxide86_core::video::render::render_ega_320x200x16(
            pixel_data,
            &self.vga_dac_palette,
            &mut image_data_buf,
        );

        let image_data =
            ImageData::new_with_u8_clamped_array_and_sh(Clamped(&image_data_buf), 640, 400)?;
        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        Ok(())
    }

    /// Render VGA graphics mode 320x200 (256-color, mode 13h) using ImageData API
    fn render_graphics_320x200x256(&mut self, pixel_data: &[u8]) -> Result<(), JsValue> {
        self.canvas.set_width(640);
        self.canvas.set_height(400);

        let mut image_data_buf = vec![0u8; 640 * 400 * 4];
        oxide86_core::video::render::render_vga_320x200x256(
            pixel_data,
            &self.vga_dac_palette,
            &mut image_data_buf,
        );

        let image_data =
            ImageData::new_with_u8_clamped_array_and_sh(Clamped(&image_data_buf), 640, 400)?;
        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        Ok(())
    }
}

impl VideoController for WebVideo {
    fn update_display(&mut self, buffer: &TextBuffer) {
        // Render all characters
        self.render_full_screen(buffer);

        // Overlay the graphical BIOS logo on top (no-op once cleared)
        self.blit_logo_overlay();

        // Flush to canvas
        if let Err(e) = self.flush_to_canvas() {
            log::error!("Failed to update display: {:?}", e);
        }
    }

    fn update_cursor(&mut self, cursor: CursorPosition) {
        // Skip cursor rendering in graphics modes - cursor is only for text mode
        if !matches!(self.current_mode, VideoMode::Text { .. }) {
            return;
        }

        // We need to redraw the character at the old cursor position to erase it,
        // then draw the new cursor. However, we don't have the buffer here.
        // For now, just store the cursor position and draw it when we update the display.
        // A more efficient implementation would store the buffer and redraw only affected cells.
        self.last_cursor = Some(cursor);

        // Draw cursor on current buffer
        self.draw_cursor(&cursor);

        // Overlay the graphical BIOS logo on top (no-op once cleared)
        self.blit_logo_overlay();

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
            0x0D => VideoMode::Graphics320x200x16,
            0x13 => VideoMode::Graphics320x200x256,
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
            VideoMode::Graphics320x200x16 => {
                // Canvas will be resized in render_graphics_320x200x16
                log::info!("WASM: Switched to 320x200x16 EGA graphics mode");
            }
            VideoMode::Graphics320x200x256 => {
                // Canvas will be resized in render_graphics_320x200x256
                log::info!("WASM: Switched to VGA mode 13h (320x200x256)");
            }
        }
    }

    fn force_redraw(&mut self, buffer: &TextBuffer) {
        // Same as update_display for web implementation
        self.update_display(buffer);
    }

    fn update_graphics_320x200(&mut self, pixel_data: &[u8], color_map: [u8; 4]) {
        // Store AC color map for rendering
        self.graphics_color_map = Some(color_map);

        if let Err(e) = self.render_graphics_320x200(pixel_data) {
            log::error!("Failed to render 320x200 graphics: {:?}", e);
        }
    }

    fn update_graphics_640x200(
        &mut self,
        pixel_data: &[u8],
        fg_color: u8,
        bg_color: u8,
        composite: bool,
    ) {
        if let Err(e) = self.render_graphics_640x200(pixel_data, fg_color, bg_color, composite) {
            log::error!("Failed to render 640x200 graphics: {:?}", e);
        }
    }

    fn update_graphics_320x200x16(&mut self, pixel_data: &[u8]) {
        if let Err(e) = self.render_graphics_320x200x16(pixel_data) {
            log::error!("Failed to render 320x200x16 EGA graphics: {:?}", e);
        }
    }

    fn update_graphics_320x200x256(&mut self, pixel_data: &[u8]) {
        if let Err(e) = self.render_graphics_320x200x256(pixel_data) {
            log::error!("Failed to render 320x200x256 VGA graphics: {:?}", e);
        }
    }

    fn update_vga_dac_palette(&mut self, palette: &[[u8; 3]; 256]) {
        // Update stored VGA DAC palette
        self.vga_dac_palette.copy_from_slice(palette);
        log::trace!("WebVideo: Updated VGA DAC palette");
    }

    fn shows_logo_overlay(&self) -> bool {
        true
    }

    fn draw_logo_overlay(&mut self, pixels: &[u8], width: usize, height: usize) {
        self.logo_overlay = Some((pixels.to_vec(), width, height));
    }

    fn clear_logo_overlay(&mut self) {
        self.logo_overlay = None;
    }
}
