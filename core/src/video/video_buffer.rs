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
            cga_composite: false,
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
        self.mode = Mode::M03Text;
        self.cga_bg = 0;
        self.cga_intensity = false;
        self.cga_palette = false;
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
        let (width, height) = self.mode.resolution();
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
            Mode::M00ColorText40
            | Mode::M01Text40
            | Mode::M02ColorText
            | Mode::M03Text => self.render_text_mode(buf),
            Mode::M04Cga320x200x4 => self.render_mode_04h_320x200x4(buf),
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
        let cols = self.text_columns as usize;
        let width = CHAR_WIDTH * cols;
        let char_height = match self.mode {
            Mode::M00ColorText40 | Mode::M01Text40 => CHAR_HEIGHT_8,
            _ => CHAR_HEIGHT,
        };

        // Render all cells
        let mut i = (self.start_address as usize) * 2;
        for row in 0..TEXT_MODE_ROWS {
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
                    },
                    buf,
                );
            }
        }

        self.render_cursor(buf, width, char_height);
    }

    fn render_cursor(&self, data: &mut [u8], width: usize, char_height: usize) {
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
                let char_y = cursor_row * char_height;

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
    /// Every 4 consecutive 1-bit pixels form a nibble (MSB = leftmost pixel). That nibble
    /// indexes into a 16-color NTSC artifact color palette, producing colors beyond B&W.
    /// `cga_palette` (bit 5 of port 0x3D9) selects the phase: false = phase 0, true = phase 1
    /// (equivalent to a 1-pixel shift, i.e. 90° NTSC colorburst phase rotation).
    /// Output is 640×400 (scanlines doubled) matching the non-composite mode 06h resolution.
    fn render_mode_06h_composite(&self, buf: &mut [u8]) {
        const SRC_WIDTH: usize = 640;
        const OUT_WIDTH: usize = 640;
        const SRC_HEIGHT: usize = 200;

        // CGA composite artifact color palettes (16 entries, RGB).
        //
        // Each 4-pixel group forms a nibble (MSB = leftmost pixel). The nibble indexes
        // into one of four palettes selected by two bits:
        //   - Bit 5 of port 0x3D9 (cga_palette): phase select.
        //     PHASE1[n] = PHASE0[rotate_left_2(n)], where rotate_left_2(n) = ((n << 2) | (n >> 2)) & 0xF
        //   - Bit 3 of bits[3:0] of port 0x3D9 (cga_bg >= 8): RGBI intensity of the foreground color.
        //     When set, selects the high-intensity variant of the palette.
        //
        // Colors calibrated to match DOSBox CGA composite output.
        const PHASE0: [[u8; 3]; 16] = [
            [0, 0, 0],       // 0  black
            [0, 75, 29],     // 1  #004B1D
            [33, 24, 155],   // 2  #21189B
            [10, 99, 166],   // 3  #0A63A6
            [104, 7, 54],    // 4  #680736
            [84, 82, 86],    // 5  #545256
            [138, 30, 170],  // 6  #8A1EAA
            [117, 106, 166], // 7  #756AA6
            [50, 58, 0],     // 8  #323A00
            [28, 136, 0],    // 9  #1C8800
            [84, 82, 86],    // 10 #545256
            [62, 159, 112],  // 11 #3E9F70
            [155, 66, 0],    // 12 #9B4200
            [133, 142, 15],  // 13 #858E0F
            [167, 90, 139],  // 14 #A75A8B
            [168, 167, 168], // 15 #A8A7A8
        ];
        const PHASE0_HI: [[u8; 3]; 16] = [
            [0, 0, 0],       // 0  black
            [0, 115, 41],    // 1  #007329
            [50, 34, 233],   // 2  #3222E9
            [18, 152, 254],  // 3  #1298FE
            [159, 9, 84],    // 4  #9F0954
            [127, 126, 127], // 5  #7F7E7F
            [210, 46, 255],  // 6  #D22EFF
            [177, 162, 255], // 7  #B1A2FF
            [74, 90, 0],     // 8  #4A5A00
            [44, 206, 0],    // 9  #2CCE00
            [127, 126, 127], // 10 #7F7E7F
            [93, 243, 168],  // 11 #5DF3A8
            [234, 100, 0],   // 12 #EA6400
            [202, 218, 19],  // 13 #CADA13
            [254, 137, 211], // 14 #FE89D3
            [255, 254, 255], // 15 #FFFEFF
        ];
        // PHASE1[n] = PHASE0[rotate_left_2(n)] — 180° subcarrier inversion.
        // Precomputed: rotate_left_2(n) = ((n << 2) | (n >> 2)) & 0xF
        const PHASE1: [[u8; 3]; 16] = [
            PHASE0[0],  // 0  → 0
            PHASE0[4],  // 1  → 4
            PHASE0[8],  // 2  → 8
            PHASE0[12], // 3  → 12
            PHASE0[1],  // 4  → 1
            PHASE0[5],  // 5  → 5
            PHASE0[9],  // 6  → 9
            PHASE0[13], // 7  → 13
            PHASE0[2],  // 8  → 2
            PHASE0[6],  // 9  → 6
            PHASE0[10], // 10 → 10
            PHASE0[14], // 11 → 14
            PHASE0[3],  // 12 → 3
            PHASE0[7],  // 13 → 7
            PHASE0[11], // 14 → 11
            PHASE0[15], // 15 → 15
        ];
        const PHASE1_HI: [[u8; 3]; 16] = [
            PHASE0_HI[0],  // 0  → 0
            PHASE0_HI[4],  // 1  → 4
            PHASE0_HI[8],  // 2  → 8
            PHASE0_HI[12], // 3  → 12
            PHASE0_HI[1],  // 4  → 1
            PHASE0_HI[5],  // 5  → 5
            PHASE0_HI[9],  // 6  → 9
            PHASE0_HI[13], // 7  → 13
            PHASE0_HI[2],  // 8  → 2
            PHASE0_HI[6],  // 9  → 6
            PHASE0_HI[10], // 10 → 10
            PHASE0_HI[14], // 11 → 14
            PHASE0_HI[3],  // 12 → 3
            PHASE0_HI[7],  // 13 → 7
            PHASE0_HI[11], // 14 → 11
            PHASE0_HI[15], // 15 → 15
        ];

        let hi = self.cga_bg >= 8;
        let palette = match (self.cga_palette, hi) {
            (false, false) => &PHASE0,
            (false, true) => &PHASE0_HI,
            (true, false) => &PHASE1,
            (true, true) => &PHASE1_HI,
        };

        for y in 0..SRC_HEIGHT {
            let bank_offset = if y % 2 == 1 { 0x2000 } else { 0 };
            let row_base = bank_offset + (y / 2) * 80;

            for group in 0..(SRC_WIDTH / 4) {
                // Collect 4 consecutive pixels into a nibble (MSB = leftmost)
                let mut nibble: usize = 0;
                for bit in 0..4 {
                    let x = group * 4 + bit;
                    let byte_val = self.vram[row_base + x / 8];
                    let pixel = (byte_val >> (7 - (x % 8))) & 1;
                    nibble = (nibble << 1) | pixel as usize;
                }
                let rgb = palette[nibble];

                // Write 4 pixels across, doubled vertically for CRT aspect ratio
                for bit in 0..4 {
                    let x = group * 4 + bit;
                    for dy in 0..2 {
                        let offset = ((y * 2 + dy) * OUT_WIDTH + x) * 4;
                        buf[offset] = rgb[0];
                        buf[offset + 1] = rgb[1];
                        buf[offset + 2] = rgb[2];
                        buf[offset + 3] = 0xFF;
                    }
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
