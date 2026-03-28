use std::fmt;

use crate::video::font::{CHAR_HEIGHT, CHAR_HEIGHT_8, CHAR_WIDTH, Cp437Font};
use crate::video::palette::TextModePalette;
use crate::video::renderer::{RenderTextArgs, dac_to_8bit, render_text};
use crate::video::text::TextAttribute;
use crate::video::{
    DEFAULT_CURSOR_END_LINE, DEFAULT_CURSOR_START_LINE, EGA_PLANE_SIZE, Mode, TEXT_MODE_COLS,
    TEXT_MODE_ROWS, TEXT_MODE_SIZE, VGA_MODE_13_HEIGHT, VGA_MODE_13_WIDTH, VIDEO_MEMORY_SIZE,
    mode::TEXT_MODE_COLS_40,
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
    mode: Mode,
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
    /// EGA/VGA Attribute Controller palette registers 0-15.
    /// Maps 4-color CGA pixel values (0-3) to VGA DAC indices for EGA/VGA cards.
    /// Only used when `ac_palette_programmed` is true; otherwise `color_map` from
    /// the CGA color select register drives the lookup.
    ac_palette: [u8; 16],
    /// Set to true the first time an AC palette register is written via port 0x3C0.
    /// Cleared on mode set so programs that don't touch the AC palette continue to
    /// use the CGA color_map path.
    ac_palette_programmed: bool,
    /// Composite (colorburst) output enabled (bit 3 of port 0x3D8).
    /// When true and mode is M06Cga640x200x2, renders NTSC artifact colors instead of B&W.
    cga_composite: bool,

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
    /// CRTC register 0x09 override (max scan line). When set, char cell height = value + 1.
    /// None = use font-based default (8 or 16 depending on mode).
    /// Reset to None on INT 10h mode set.
    crtc_max_scan_line: Option<u8>,
    /// CRTC register 0x06 override (vertical displayed rows).
    /// None = use default 25 rows.
    /// Reset to None on INT 10h mode set.
    crtc_vertical_displayed: Option<u8>,
    /// CRTC register 0x13 (Offset): logical scan-line stride in words (2 bytes each).
    /// bytes_per_row = crtc_offset * 2.  None = use mode default.
    /// Reset to None on INT 10h mode set.
    crtc_offset: Option<u8>,
    /// If any value changes in the struct which could result in different output this will be set to true
    dirty: bool,
}

impl VideoBuffer {
    pub fn new() -> Self {
        let mut me = Self {
            mode: Mode::M03Text,
            text_columns: TEXT_MODE_COLS as u8,
            vram: vec![0; VIDEO_MEMORY_SIZE],
            cga_bg: 0,
            cga_intensity: false,
            cga_palette: false,
            ac_palette: core::array::from_fn(|i| i as u8),
            ac_palette_programmed: false,
            cga_composite: false,
            font: Cp437Font::new(),
            vga_dac_palette: Self::default_vga_dac_palette(),
            blink_enabled: false,
            cursor_loc: 0,
            cursor_start_line: DEFAULT_CURSOR_START_LINE,
            cursor_visible: true,
            cursor_end_line: DEFAULT_CURSOR_END_LINE,
            start_address: 0,
            crtc_max_scan_line: None,
            crtc_vertical_displayed: None,
            crtc_offset: None,
            dirty: false,
        };
        me.reset();
        me
    }

    pub(crate) fn reset(&mut self) {
        self.mode = Mode::M03Text;
        self.cga_bg = 0;
        self.cga_intensity = false;
        self.cga_palette = false;
        self.ac_palette = core::array::from_fn(|i| i as u8);
        self.ac_palette_programmed = false;
        self.cga_composite = false;
        self.text_columns = TEXT_MODE_COLS as u8;
        self.font = Cp437Font::new();
        self.vga_dac_palette = Self::default_vga_dac_palette();
        self.blink_enabled = false;
        self.cursor_loc = 0;
        self.cursor_start_line = DEFAULT_CURSOR_START_LINE;
        self.cursor_visible = true;
        self.cursor_end_line = DEFAULT_CURSOR_END_LINE;
        self.start_address = 0;
        self.crtc_max_scan_line = None;
        self.crtc_vertical_displayed = None;
        self.crtc_offset = None;
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

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.text_columns = match &mode {
            Mode::M00ColorText40 | Mode::M01Text40 => TEXT_MODE_COLS_40 as u8,
            _ => TEXT_MODE_COLS as u8,
        };
        // MDA mode 7: bit 7 of the attribute byte is always the blink flag.
        // CGA modes 0-6 default to intensity mode (bit 7 = bright background).
        self.blink_enabled = matches!(mode, Mode::M07MdaText);
        self.mode = mode;
        // Reset the AC palette so that a fresh mode set without subsequent
        // AC reprogramming falls back to the CGA color_map path.
        self.ac_palette = core::array::from_fn(|i| i as u8);
        self.ac_palette_programmed = false;
    }

    /// Reset CRTC timing overrides back to hardware defaults.
    /// Called when INT 10h AH=0 sets a new video mode (which reprograms the CRTC).
    /// Direct I/O writes to 0x3D8 do NOT call this.
    pub(crate) fn reset_crtc_overrides(&mut self) {
        self.crtc_max_scan_line = None;
        self.crtc_vertical_displayed = None;
        self.crtc_offset = None;
        self.dirty = true;
    }

    pub(crate) fn set_crtc_max_scan_line(&mut self, val: u8) {
        self.crtc_max_scan_line = Some(val);
        self.dirty = true;
    }

    pub(crate) fn set_crtc_vertical_displayed(&mut self, val: u8) {
        self.crtc_vertical_displayed = Some(val);
        self.dirty = true;
    }

    pub(crate) fn set_crtc_offset(&mut self, val: u8) {
        self.crtc_offset = Some(val);
        self.dirty = true;
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

    pub(crate) fn set_dac_color(&mut self, index: usize, r: u8, g: u8, b: u8) {
        self.vga_dac_palette[index] = [r, g, b];
        self.dirty = true;
    }

    pub(crate) fn set_cga_color_select(&mut self, bg: usize, intensity: bool, palette: bool) {
        self.cga_bg = bg;
        self.cga_intensity = intensity;
        self.cga_palette = palette;
        self.dirty = true;
    }

    /// Record an EGA/VGA AC palette register write.  Once called, the AC
    /// palette takes over CGA 4-color pixel → DAC-index translation in
    /// `render_mode_04h_320x200x4`, overriding the hardcoded `color_map`.
    pub(crate) fn set_ac_palette_register(&mut self, index: usize, value: u8) {
        if index < 16 {
            self.ac_palette[index] = value;
            self.ac_palette_programmed = true;
            self.dirty = true;
        }
    }

    pub(crate) fn set_cga_composite(&mut self, enabled: bool) {
        self.cga_composite = enabled;
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn render_and_clear_dirty(&mut self, buf: &mut [u8]) {
        self.render_into(buf);
        self.dirty = false;
    }

    pub fn render(&self) -> RenderResult {
        let (width, height) = self.render_resolution();
        let mut data = vec![0u8; width as usize * height as usize * 4];
        self.render_into(&mut data);
        RenderResult {
            data,
            width,
            height,
        }
    }

    fn render_resolution(&self) -> (u32, u32) {
        if self.crtc_max_scan_line.is_some() || self.crtc_vertical_displayed.is_some() {
            // Always output at the standard CGA text resolution so scanlines fill the
            // same physical height as normal text mode (matching mode 06h behaviour).
            let cols = self.text_columns as usize;
            return (
                (CHAR_WIDTH * cols) as u32,
                (CHAR_HEIGHT * TEXT_MODE_ROWS) as u32,
            );
        }
        self.mode.resolution()
    }

    /// Returns (cols, rows, char_height) for text mode rendering,
    /// honouring any active CRTC overrides.
    fn text_geometry(&self) -> (usize, usize, usize) {
        let cols = self.text_columns as usize;
        let char_height = if let Some(max_sl) = self.crtc_max_scan_line {
            (max_sl as usize) + 1
        } else {
            match self.mode {
                Mode::M00ColorText40 | Mode::M01Text40 => CHAR_HEIGHT_8,
                _ => CHAR_HEIGHT,
            }
        };
        let rows = self.crtc_vertical_displayed.unwrap_or(TEXT_MODE_ROWS as u8) as usize;
        (cols, rows, char_height)
    }

    fn render_into(&self, buf: &mut [u8]) {
        match self.mode {
            Mode::M00ColorText40
            | Mode::M01Text40
            | Mode::M02ColorText
            | Mode::M03Text
            | Mode::M07MdaText => self.render_text_mode(buf),
            Mode::M04Cga320x200x4 | Mode::M05Cga320x200x4 => self.render_mode_04h_320x200x4(buf),
            Mode::M06Cga640x200x2 => {
                if self.cga_composite {
                    self.render_mode_06h_composite(buf)
                } else {
                    self.render_mode_06h_640x200x2(buf)
                }
            }
            Mode::M0DEga320x200x16 => self.render_mode_0dh_320x200x16(buf),
            Mode::M10Ega640x350x16 => self.render_mode_10h_640x350x16(buf),
            Mode::M13Vga320x200x256 => self.render_mode_13h_320x200x256(buf),
            Mode::Unknown(_) => self.render_text_mode(buf),
        }
    }

    fn render_text_mode(&self, buf: &mut [u8]) {
        let (cols, rows, char_height) = self.text_geometry();
        let width = CHAR_WIDTH * cols;

        // When CRTC overrides are active the output is padded to standard text height
        // (640×400) by scaling each source scanline.  For unmodified text modes the
        // mode resolution already matches the buffer so no scaling is needed.
        let scanline_scale =
            if self.crtc_max_scan_line.is_some() || self.crtc_vertical_displayed.is_some() {
                let standard_height = CHAR_HEIGHT * TEXT_MODE_ROWS;
                let render_height = rows * char_height;
                if render_height > 0 && render_height < standard_height {
                    standard_height / render_height
                } else {
                    1
                }
            } else {
                1
            };

        // Render all cells
        let mut i = (self.start_address as usize) * 2;
        for row in 0..rows {
            for col in 0..cols {
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
                        char_height,
                        scanline_scale,
                    },
                    buf,
                );
            }
        }

        self.render_cursor(buf, width, char_height, scanline_scale);
    }

    fn render_cursor(
        &self,
        data: &mut [u8],
        width: usize,
        char_height: usize,
        scanline_scale: usize,
    ) {
        if self.cursor_visible {
            let cursor_row = (self.cursor_loc / self.text_columns as u16) as usize;
            let cursor_col = (self.cursor_loc % self.text_columns as u16) as usize;

            if cursor_row < TEXT_MODE_ROWS && cursor_col < self.text_columns as usize {
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
                let end_scan = (self.cursor_end_line as usize).min(char_height - 1);

                let char_x = cursor_col * CHAR_WIDTH;
                let char_y = cursor_row * char_height * scanline_scale;

                for scan_line in start_scan..=end_scan {
                    for s in 0..scanline_scale {
                        let pixel_y = char_y + scan_line * scanline_scale + s;
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

        // Map 2-bit pixel values to EGA color indices (0-15).
        // Mode 5 ignores the palette select bit and always uses the alt color set
        // (cyan/red/grey), matching real CGA hardware where colorburst is disabled.
        let color_map: [usize; 4] = if matches!(self.mode, Mode::M05Cga320x200x4) {
            match intensity {
                false => [bg, 3, 4, 7],   // cyan, red, grey
                true => [bg, 11, 12, 15], // light cyan, light red, white
            }
        } else {
            match (palette, intensity) {
                (false, false) => [bg, 2, 4, 6],   // green, red, brown
                (false, true) => [bg, 10, 12, 14], // light green, light red, yellow
                (true, false) => [bg, 3, 5, 7],    // cyan, magenta, white
                (true, true) => [bg, 11, 13, 15],  // light cyan, light magenta, bright white
            }
        };

        for y in 0..HEIGHT {
            let bank_offset = if y % 2 == 1 { 0x2000 } else { 0 };
            for x in 0..WIDTH {
                let byte_offset = bank_offset + (y / 2) * 80 + x / 4;
                let shift = 6 - (x % 4) * 2;
                let color_index = ((self.vram[byte_offset] >> shift) & 0x03) as usize;

                // When the AC palette has been explicitly programmed (EGA/VGA cards),
                // use it to resolve the 2-bit pixel value to a DAC index.  Programs
                // like Checkit reprogram both the AC palette and DAC registers together,
                // so the hardcoded color_map would point at the wrong DAC entries.
                let dac_index = if self.ac_palette_programmed {
                    self.ac_palette[color_index] as usize
                } else {
                    color_map[color_index]
                };
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
        let bytes_per_row = self.crtc_offset.map(|v| v as usize * 2).unwrap_or(40);
        let start_byte = self.start_address as usize;

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let byte_offset = (start_byte + y * bytes_per_row + x / 8) % EGA_PLANE_SIZE;
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
        let bytes_per_row = self.crtc_offset.map(|v| v as usize * 2).unwrap_or(80);
        let start_byte = self.start_address as usize;

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let byte_offset = (start_byte + y * bytes_per_row + x / 8) % EGA_PLANE_SIZE;
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

    /// Render VGA 320x200 256-color graphics (mode 13h).
    ///
    /// Linear framebuffer: vram[0..VGA_MODE_13_FRAMEBUFFER_SIZE], 1 byte per pixel (palette index).
    /// Each index is looked up in the VGA DAC palette to produce an RGB value.
    fn render_mode_13h_320x200x256(&self, buf: &mut [u8]) {
        for y in 0..VGA_MODE_13_HEIGHT {
            for x in 0..VGA_MODE_13_WIDTH {
                let color_index = self.vram[y * VGA_MODE_13_WIDTH + x] as usize;
                let dac = self.vga_dac_palette[color_index];
                let offset = (y * VGA_MODE_13_WIDTH + x) * 4;
                buf[offset] = dac_to_8bit(dac[0]);
                buf[offset + 1] = dac_to_8bit(dac[1]);
                buf[offset + 2] = dac_to_8bit(dac[2]);
                buf[offset + 3] = 0xFF;
            }
        }
    }

    /// Render CGA 640x200 mode 06h in composite (colorburst) mode.
    ///
    /// Simulates NTSC composite color decoding via a 4-pixel sliding window filter.
    /// Each pixel's color is derived by accumulating luma (Y) and chroma (I, Q) signals
    /// from a 4-sample window, then converting YIQ→RGB. The HUE_SETTING constant controls
    /// the color clock phase (tint knob).
    /// Output is 640×400 (scanlines doubled) matching the non-composite mode 06h resolution.
    fn render_mode_06h_composite(&self, buf: &mut [u8]) {
        const SCREEN_WIDTH: usize = 640;
        const SCREEN_HEIGHT: usize = 200;
        const VRAM_BANK_SIZE: usize = 0x2000; // 8 KB per bank

        // Brightness scale (1.0 = full, lower values dim the output).
        const BRIGHTNESS: f32 = 0.7;
        const HUE: f32 = 309.0;

        // Hardware base phase of the CGA color clock.
        // cga_intensity (bit 4 of port 0x3D9) inverts the colorburst phase by 180°.
        let intensity_phase = if self.cga_intensity { 180.0 } else { 0.0 };
        let hue_setting = (HUE + intensity_phase) % 360.0;
        let brightness = if self.cga_bg & 0x08 == 0 {
            BRIGHTNESS
        } else {
            1.0
        };

        // Pre-compute cos/sin phase for the 4-pixel NTSC color cycle (x % 4 repeats).
        let phase_lookup_cos: [f32; 4] = std::array::from_fn(|i| {
            let angle = (i as f32 * 90.0 + hue_setting) * std::f32::consts::PI / 180.0;
            angle.cos()
        });
        let phase_lookup_sin: [f32; 4] = std::array::from_fn(|i| {
            let angle = (i as f32 * 90.0 + hue_setting) * std::f32::consts::PI / 180.0;
            angle.sin()
        });

        let mut scanline_bits = [0u8; SCREEN_WIDTH];

        for y in 0..SCREEN_HEIGHT {
            // CGA interlacing: even lines at bank 0 (0x0000), odd lines at bank 1 (0x2000).
            let bank_offset = (y % 2) * VRAM_BANK_SIZE;
            let line_offset = (y / 2) * 80; // 80 bytes per row (640 pixels / 8 bpp)

            // Unpack all 640 bits for this scanline (MSB = leftmost pixel).
            for b in 0..80 {
                let byte_val = self.vram[bank_offset + line_offset + b];
                for bit in 0..8 {
                    scanline_bits[b * 8 + bit] = (byte_val >> (7 - bit)) & 1;
                }
            }

            for x in 0..SCREEN_WIDTH {
                let mut y_luma: f32 = 0.0;
                let mut i_chroma: f32 = 0.0;
                let mut q_chroma: f32 = 0.0;

                // 4-pixel sliding window — matches NTSC color subcarrier cycle length.
                for w in 0..4 {
                    let sample_x = x + w;
                    if sample_x < SCREEN_WIDTH {
                        let pixel = scanline_bits[sample_x] as f32;
                        y_luma += pixel;
                        i_chroma += pixel * phase_lookup_cos[sample_x % 4];
                        q_chroma += pixel * phase_lookup_sin[sample_x % 4];
                    }
                }

                let yy = (y_luma / 4.0) * brightness;
                // Chroma amplitude: real CGA chroma is ~40% of luma range.
                // Dividing by 5 (vs luma /4) keeps chroma from clamping artifact colors.
                let ii = i_chroma / 5.0;
                let qq = q_chroma / 5.0;

                // YIQ → RGB conversion matrix.
                let r = (yy + 0.956 * ii + 0.621 * qq).clamp(0.0, 1.0);
                let g = (yy - 0.272 * ii - 0.647 * qq).clamp(0.0, 1.0);
                let b = (yy - 1.106 * ii + 1.703 * qq).clamp(0.0, 1.0);

                let r8 = (r * 255.0) as u8;
                let g8 = (g * 255.0) as u8;
                let b8 = (b * 255.0) as u8;

                // Double scanlines for CRT aspect ratio (640×200 → 640×400).
                for dy in 0..2 {
                    let offset = ((y * 2 + dy) * SCREEN_WIDTH + x) * 4;
                    buf[offset] = r8;
                    buf[offset + 1] = g8;
                    buf[offset + 2] = b8;
                    buf[offset + 3] = 0xFF;
                }
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

        let fg_dac = self.vga_dac_palette[self.cga_bg]; // foreground color from color select register
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
