use std::fmt;

use crate::video::font::{CHAR_HEIGHT, CHAR_WIDTH, Cp437Font};
use crate::video::palette::TextModePalette;
use crate::video::renderer::{RenderTextArgs, dac_to_8bit, render_text};
use crate::video::text::TextAttribute;
use crate::video::{
    DEFAULT_CURSOR_END_LINE, DEFAULT_CURSOR_START_LINE, EGA_PLANE_SIZE, TEXT_MODE_COLS,
    TEXT_MODE_ROWS, TEXT_MODE_SIZE, VIDEO_MEMORY_SIZE, VIDEO_MODE_0DH_EGA_320_X_200_16,
    VIDEO_MODE_02H_COLOR_TEXT_80_X_25, VIDEO_MODE_03H_COLOR_TEXT_80_X_25,
    VIDEO_MODE_04H_CGA_320_X_200_4, VIDEO_MODE_06H_CGA_640_X_200_2,
    VIDEO_MODE_10H_EGA_640_X_350_16,
};

#[derive(PartialEq)]
pub struct RenderResult {
    /// RGBA data
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Cursor position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorPosition {
    pub row: u8,
    pub col: u8,
}

impl fmt::Display for CursorPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{},{}", self.col, self.row)
    }
}

pub struct VideoBuffer {
    mode: u8,
    text_columns: u8,

    /// Raw video RAM (64KB).
    /// In CGA/text modes: framebuffer at B8000-BFFFF.
    /// In EGA mode 0x0D: 4 planes × EGA_PLANE_SIZE bytes (plane N at vram[N*EGA_PLANE_SIZE..]).
    /// In VGA mode 0x13: linear framebuffer vram[0..64000], 1 byte per pixel.
    /// Persists across mode changes, just like real hardware.
    vram: Vec<u8>,

    /// Background color index (bits 3:0 of port 0x3D9). Used as color 0 in 4-color graphics modes.
    cga_bg: usize,
    /// High-intensity colors (bit 4 of port 0x3D9). Selects bright variants of the active palette.
    cga_intensity: bool,
    /// Palette select (bit 5 of port 0x3D9). false = green/red/brown, true = cyan/magenta/white.
    cga_palette: bool,

    font: Cp437Font,
    /// VGA DAC palette registers (256 entries, each with 6-bit RGB components)
    vga_dac_palette: [[u8; 3]; 256],
    /// Blink/intensity mode for text attribute bit 7.
    /// true  = bit 7 enables character blinking (8 background colors, default)
    /// false = bit 7 selects high-intensity background (16 background colors, no blink)
    blink_enabled: bool,
    /// Cursor position as a character cell index, written by CRT controller
    /// registers 0x0E (high byte) and 0x0F (low byte). Row-major within the
    /// current text mode grid: col = loc % cols, row = loc / cols.
    cursor_loc: u16,
    /// Scan line within a character cell where the cursor begins, written by
    /// CRT controller register 0x0A (bits 4:0). Together with `cursor_end_line`
    /// this defines the vertical extent of the cursor block (0 = top of cell).
    cursor_start_line: u8,
    /// Whether the cursor is visible. Derived from bit 5 (0x20) of CRT register 0x0A:
    /// when that bit is set the cursor is hidden; when clear the cursor is visible.
    cursor_visible: bool,
    /// Scan line within a character cell where the cursor ends (inclusive),
    /// written by CRT controller register 0x0B (bits 4:0). A value equal to
    /// `CHAR_HEIGHT - 1` produces an underline cursor at the bottom of the cell.
    cursor_end_line: u8,
    /// Display start address written by CRT controller registers 0x0C (high) and 0x0D (low).
    /// Controls which VRAM offset is shown at the top-left of the screen (used for hardware scrolling).
    start_address: u16,
    /// If any value changes in the struct which could result in different output this will be set to true
    dirty: bool,
}

impl VideoBuffer {
    pub fn new() -> Self {
        let mut me = Self {
            mode: VIDEO_MODE_03H_COLOR_TEXT_80_X_25,
            text_columns: TEXT_MODE_COLS as u8,
            vram: vec![0; VIDEO_MEMORY_SIZE],
            cga_bg: 0,
            cga_intensity: false,
            cga_palette: false,
            font: Cp437Font::new(),
            vga_dac_palette: Self::default_vga_dac_palette(),
            blink_enabled: false,
            cursor_loc: 0,
            cursor_start_line: DEFAULT_CURSOR_START_LINE,
            cursor_visible: true,
            cursor_end_line: DEFAULT_CURSOR_END_LINE,
            start_address: 0,
            dirty: false,
        };
        me.reset();
        me
    }

    pub(crate) fn reset(&mut self) {
        self.mode = VIDEO_MODE_03H_COLOR_TEXT_80_X_25;
        self.cga_bg = 0;
        self.cga_intensity = false;
        self.cga_palette = false;
        self.text_columns = TEXT_MODE_COLS as u8;
        self.font = Cp437Font::new();
        self.vga_dac_palette = Self::default_vga_dac_palette();
        self.blink_enabled = false;
        self.cursor_loc = 0;
        self.cursor_start_line = DEFAULT_CURSOR_START_LINE;
        self.cursor_visible = true;
        self.cursor_end_line = DEFAULT_CURSOR_END_LINE;
        self.start_address = 0;
        self.dirty = false;
        for i in (0..TEXT_MODE_SIZE).step_by(2) {
            self.vram[i] = 0x20; // space
            self.vram[i + 1] = 0x07; // Light Gray on Black
        }
    }

    /// Initialize VGA DAC palette with EGA defaults
    fn default_vga_dac_palette() -> [[u8; 3]; 256] {
        let mut palette = [[0u8; 3]; 256];
        // Initialize first 16 colors with EGA defaults (6-bit RGB values 0-63)
        for (i, entry) in palette.iter_mut().enumerate().take(16) {
            *entry = TextModePalette::get_dac_color(i as u8);
        }
        palette
    }

    pub fn mode(&self) -> u8 {
        self.mode
    }

    pub fn set_mode(&mut self, mode: u8) {
        self.mode = mode;
    }

    pub fn read_vram(&self, addr: usize) -> u8 {
        self.vram[addr]
    }

    pub(crate) fn write_vram(&mut self, addr: usize, val: u8) {
        self.vram[addr] = val;
        self.dirty = true;
    }

    pub(crate) fn vram_len(&self) -> usize {
        self.vram.len()
    }

    pub(crate) fn cursor_loc(&self) -> u16 {
        self.cursor_loc
    }

    pub(crate) fn cursor_start_line(&self) -> u8 {
        self.cursor_start_line
    }

    pub(crate) fn cursor_end_line(&self) -> u8 {
        self.cursor_end_line
    }

    pub(crate) fn set_cursor_loc(&mut self, loc: u16) {
        self.cursor_loc = loc;
        self.dirty = true;
    }

    pub(crate) fn set_cursor_start_line(&mut self, start_line: u8) {
        log::debug!("set_cursor_start_line {start_line}");
        self.cursor_start_line = start_line;
        self.dirty = true;
    }

    pub(crate) fn set_cursor_visible(&mut self, visible: bool) {
        log::debug!("set_cursor_visible {visible}");
        self.cursor_visible = visible;
        self.dirty = true;
    }

    pub(crate) fn set_cursor_end_line(&mut self, end_line: u8) {
        log::debug!("set_cursor_end_line {end_line}");
        self.cursor_end_line = end_line;
        self.dirty = true;
    }

    pub(crate) fn start_address(&self) -> u16 {
        self.start_address
    }

    pub(crate) fn set_start_address(&mut self, addr: u16) {
        self.start_address = addr;
        self.dirty = true;
    }

    pub fn get_cursor_position(&self) -> CursorPosition {
        CursorPosition {
            row: (self.cursor_loc / self.text_columns as u16) as u8,
            col: (self.cursor_loc % self.text_columns as u16) as u8,
        }
    }

    pub fn blink_enabled(&self) -> bool {
        self.blink_enabled
    }

    pub fn vga_dac_palette(&self) -> &[[u8; 3]; 256] {
        &self.vga_dac_palette
    }

    pub(crate) fn set_cga_color_select(&mut self, bg: usize, intensity: bool, palette: bool) {
        self.cga_bg = bg;
        self.cga_intensity = intensity;
        self.cga_palette = palette;
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn resolution(&self) -> (u32, u32) {
        match self.mode {
            VIDEO_MODE_04H_CGA_320_X_200_4 => (320, 200),
            VIDEO_MODE_06H_CGA_640_X_200_2 => (640, 400),
            VIDEO_MODE_0DH_EGA_320_X_200_16 => (320, 200),
            VIDEO_MODE_10H_EGA_640_X_350_16 => (640, 350),
            _ => (
                (CHAR_WIDTH * TEXT_MODE_COLS) as u32,
                (CHAR_HEIGHT * TEXT_MODE_ROWS) as u32,
            ),
        }
    }

    pub fn render_and_clear_dirty(&mut self, buf: &mut [u8]) {
        self.render_into(buf);
        self.dirty = false;
    }

    pub fn render(&self) -> RenderResult {
        let (width, height) = self.resolution();
        let mut data = vec![0u8; width as usize * height as usize * 4];
        self.render_into(&mut data);
        RenderResult {
            data,
            width,
            height,
        }
    }

    fn render_into(&self, buf: &mut [u8]) {
        match self.mode {
            VIDEO_MODE_02H_COLOR_TEXT_80_X_25 | VIDEO_MODE_03H_COLOR_TEXT_80_X_25 => {
                self.render_text_mode(buf)
            }
            VIDEO_MODE_04H_CGA_320_X_200_4 => self.render_mode_04h_320x200x4(buf),
            VIDEO_MODE_06H_CGA_640_X_200_2 => self.render_mode_06h_640x200x2(buf),
            VIDEO_MODE_0DH_EGA_320_X_200_16 => self.render_mode_0dh_320x200x16(buf),
            VIDEO_MODE_10H_EGA_640_X_350_16 => self.render_mode_10h_640x350x16(buf),
            _ => self.render_text_mode(buf),
        }
    }

    fn render_text_mode(&self, buf: &mut [u8]) {
        let width = CHAR_WIDTH * TEXT_MODE_COLS;

        // Render all cells
        let mut i = (self.start_address as usize) * 2;
        for row in 0..TEXT_MODE_ROWS {
            for col in 0..TEXT_MODE_COLS {
                let character = self.vram[i];
                i += 1;
                let text_attr = TextAttribute::from_byte(self.vram[i], self.blink_enabled);
                i += 1;
                render_text(
                    RenderTextArgs {
                        font: &self.font,
                        row,
                        col,
                        character,
                        text_attr,
                        vga_dac_palette: &self.vga_dac_palette,
                        stride: width,
                    },
                    buf,
                );
            }
        }

        self.render_cursor(buf, width);
    }

    fn render_cursor(&self, data: &mut [u8], width: usize) {
        if self.cursor_visible {
            let cursor_row = (self.cursor_loc / self.text_columns as u16) as usize;
            let cursor_col = (self.cursor_loc % self.text_columns as u16) as usize;

            if cursor_row < TEXT_MODE_ROWS && cursor_col < TEXT_MODE_COLS {
                // Use foreground color of the character cell under the cursor
                let cell_idx = (cursor_row * self.text_columns as usize + cursor_col) * 2;
                let attr_byte = self.vram[cell_idx + 1];
                let text_attr = TextAttribute::from_byte(attr_byte, self.blink_enabled);
                let fg_dac = self.vga_dac_palette[text_attr.foreground as usize];
                let fg_color = [
                    dac_to_8bit(fg_dac[0]),
                    dac_to_8bit(fg_dac[1]),
                    dac_to_8bit(fg_dac[2]),
                ];

                // cursor_start_line and cursor_end_line are direct scan-line indices
                // within the character cell (0-based). Use them as-is, clamped to the
                // render height.
                let start_scan = self.cursor_start_line as usize;
                let end_scan = (self.cursor_end_line as usize).min(CHAR_HEIGHT - 1);

                let char_x = cursor_col * CHAR_WIDTH;
                let char_y = cursor_row * CHAR_HEIGHT;

                for scan_line in start_scan..=end_scan {
                    let pixel_y = char_y + scan_line;
                    for bit in 0..CHAR_WIDTH {
                        let pixel_x = char_x + bit;
                        let offset = (pixel_y * width + pixel_x) * 4;
                        data[offset] = fg_color[0];
                        data[offset + 1] = fg_color[1];
                        data[offset + 2] = fg_color[2];
                        data[offset + 3] = 0xFF;
                    }
                }
            }
        }
    }

    /// Render CGA 320x200 4-color graphics (mode 04h).
    ///
    /// CGA VRAM is interleaved: even scan lines at 0x0000, odd scan lines at 0x2000.
    /// Each pixel is 2 bits (4 pixels per byte). The CGA color select register
    /// (port 0x3D9) determines the palette: bits 3:0 = background color,
    /// bit 4 = intensity, bit 5 = palette (0=green/red/yellow, 1=cyan/magenta/white).
    fn render_mode_04h_320x200x4(&self, buf: &mut [u8]) {
        const WIDTH: usize = 320;
        const HEIGHT: usize = 200;

        let bg = self.cga_bg;
        let intensity = self.cga_intensity;
        let palette = self.cga_palette;

        // Map 2-bit pixel values to EGA color indices (0-15)
        let color_map: [usize; 4] = match (palette, intensity) {
            (false, false) => [bg, 2, 4, 6],   // green, red, brown
            (false, true) => [bg, 10, 12, 14], // light green, light red, yellow
            (true, false) => [bg, 3, 5, 7],    // cyan, magenta, white
            (true, true) => [bg, 11, 13, 15],  // light cyan, light magenta, bright white
        };

        for y in 0..HEIGHT {
            let bank_offset = if y % 2 == 1 { 0x2000 } else { 0 };
            for x in 0..WIDTH {
                let byte_offset = bank_offset + (y / 2) * 80 + x / 4;
                let shift = 6 - (x % 4) * 2;
                let color_index = ((self.vram[byte_offset] >> shift) & 0x03) as usize;

                let dac_index = color_map[color_index];
                let dac = self.vga_dac_palette[dac_index];
                let rgb = [
                    dac_to_8bit(dac[0]),
                    dac_to_8bit(dac[1]),
                    dac_to_8bit(dac[2]),
                ];

                let offset = (y * WIDTH + x) * 4;
                buf[offset] = rgb[0];
                buf[offset + 1] = rgb[1];
                buf[offset + 2] = rgb[2];
                buf[offset + 3] = 0xFF;
            }
        }
    }

    /// Render EGA 320x200 16-color graphics (mode 0Dh).
    ///
    /// EGA VRAM is planar: 4 planes, each EGA_PLANE_SIZE bytes, stored sequentially.
    /// Each pixel is 4 bits (1 bit per plane). The pixel color index is:
    ///   color = plane3_bit<<3 | plane2_bit<<2 | plane1_bit<<1 | plane0_bit
    /// 40 bytes per row × 200 rows, 8 pixels per byte.
    fn render_mode_0dh_320x200x16(&self, buf: &mut [u8]) {
        const WIDTH: usize = 320;
        const HEIGHT: usize = 200;

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let byte_offset = y * 40 + x / 8;
                let bit_pos = 7 - (x % 8);

                let mut color_index: usize = 0;
                for plane in 0..4usize {
                    let plane_byte = self.vram[plane * EGA_PLANE_SIZE + byte_offset];
                    if (plane_byte >> bit_pos) & 1 != 0 {
                        color_index |= 1 << plane;
                    }
                }

                let dac = self.vga_dac_palette[color_index];
                let offset = (y * WIDTH + x) * 4;
                buf[offset] = dac_to_8bit(dac[0]);
                buf[offset + 1] = dac_to_8bit(dac[1]);
                buf[offset + 2] = dac_to_8bit(dac[2]);
                buf[offset + 3] = 0xFF;
            }
        }
    }

    /// Render EGA 640x350 16-color graphics (mode 10h).
    ///
    /// Same planar layout as mode 0Dh but 80 bytes per row × 350 rows.
    /// Each pixel is 4 bits (1 bit per plane). The pixel color index is:
    ///   color = plane3_bit<<3 | plane2_bit<<2 | plane1_bit<<1 | plane0_bit
    fn render_mode_10h_640x350x16(&self, buf: &mut [u8]) {
        const WIDTH: usize = 640;
        const HEIGHT: usize = 350;
        const BYTES_PER_ROW: usize = 80;

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let byte_offset = y * BYTES_PER_ROW + x / 8;
                let bit_pos = 7 - (x % 8);

                let mut color_index: usize = 0;
                for plane in 0..4usize {
                    let plane_byte = self.vram[plane * EGA_PLANE_SIZE + byte_offset];
                    if (plane_byte >> bit_pos) & 1 != 0 {
                        color_index |= 1 << plane;
                    }
                }

                let dac = self.vga_dac_palette[color_index];
                let offset = (y * WIDTH + x) * 4;
                buf[offset] = dac_to_8bit(dac[0]);
                buf[offset + 1] = dac_to_8bit(dac[1]);
                buf[offset + 2] = dac_to_8bit(dac[2]);
                buf[offset + 3] = 0xFF;
            }
        }
    }

    /// Render CGA 640x200 monochrome graphics (mode 06h).
    ///
    /// CGA VRAM is interleaved: even scan lines at 0x0000, odd scan lines at 0x2000.
    /// Each pixel is 1 bit (8 pixels per byte). Background is black, foreground is white.
    /// Scanlines are doubled to 640x400 to approximate the 4:3 CRT aspect ratio,
    /// since real CGA monitors displayed 640x200 with non-square pixels (~2× taller).
    fn render_mode_06h_640x200x2(&self, buf: &mut [u8]) {
        const WIDTH: usize = 640;
        const SRC_HEIGHT: usize = 200;

        let fg_dac = self.vga_dac_palette[15]; // white
        let bg_dac = self.vga_dac_palette[0]; // black
        let fg_rgb = [
            dac_to_8bit(fg_dac[0]),
            dac_to_8bit(fg_dac[1]),
            dac_to_8bit(fg_dac[2]),
        ];
        let bg_rgb = [
            dac_to_8bit(bg_dac[0]),
            dac_to_8bit(bg_dac[1]),
            dac_to_8bit(bg_dac[2]),
        ];

        for y in 0..SRC_HEIGHT {
            let bank_offset = if y % 2 == 1 { 0x2000 } else { 0 };
            for x in 0..WIDTH {
                let byte_val = self.vram[bank_offset + (y / 2) * 80 + x / 8];
                let bit_mask = 0x80u8 >> (x % 8);
                let rgb = if (byte_val & bit_mask) != 0 {
                    fg_rgb
                } else {
                    bg_rgb
                };

                // Double each scanline for correct CRT aspect ratio
                for dy in 0..2 {
                    let offset = ((y * 2 + dy) * WIDTH + x) * 4;
                    buf[offset] = rgb[0];
                    buf[offset + 1] = rgb[1];
                    buf[offset + 2] = rgb[2];
                    buf[offset + 3] = 0xFF;
                }
            }
        }
    }
}
