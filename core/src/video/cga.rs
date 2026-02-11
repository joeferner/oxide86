use crate::colors;

/// Graphics framebuffer for CGA modes
pub struct CgaBuffer {
    /// Raw pixel data (16KB for CGA modes)
    /// Interlaced: first 8KB = even scan lines, second 8KB = odd scan lines
    data: Vec<u8>,
    /// Width in pixels
    width: usize,
    /// Height in pixels
    #[allow(dead_code)]
    height: usize,
    /// Bits per pixel (1 for 640x200, 2 for 320x200)
    bits_per_pixel: u8,
}

impl CgaBuffer {
    pub fn new_320x200() -> Self {
        Self {
            data: vec![0; 16000], // 320x200 / 4 pixels per byte
            width: 320,
            height: 200,
            bits_per_pixel: 2,
        }
    }

    pub fn new_640x200() -> Self {
        Self {
            data: vec![0; 16000], // 640x200 / 8 pixels per byte
            width: 640,
            height: 200,
            bits_per_pixel: 1,
        }
    }

    /// Convert linear framebuffer offset to interlaced CGA memory offset
    /// CGA uses interlaced memory: even lines at 0x0000-0x1F3F, odd at 0x2000-0x3F3F
    #[allow(dead_code)]
    fn linear_to_interlaced(&self, offset: usize) -> usize {
        let bytes_per_line = self.width * (self.bits_per_pixel as usize) / 8;
        let line = offset / bytes_per_line;
        let col = offset % bytes_per_line;

        if line.is_multiple_of(2) {
            // Even line: bank 0 (0x0000-0x1F3F)
            (line / 2) * bytes_per_line + col
        } else {
            // Odd line: bank 1 (0x2000-0x3F3F), offset by 8KB
            0x2000 + (line / 2) * bytes_per_line + col
        }
    }

    /// Convert interlaced CGA memory offset to linear framebuffer offset
    fn interlaced_to_linear(&self, offset: usize) -> usize {
        let bytes_per_line = self.width * (self.bits_per_pixel as usize) / 8;

        if offset < 0x2000 {
            // Even line bank
            let line_in_bank = offset / bytes_per_line;
            let col = offset % bytes_per_line;
            (line_in_bank * 2) * bytes_per_line + col
        } else {
            // Odd line bank
            let offset_in_bank = offset - 0x2000;
            let line_in_bank = offset_in_bank / bytes_per_line;
            let col = offset_in_bank % bytes_per_line;
            (line_in_bank * 2 + 1) * bytes_per_line + col
        }
    }

    /// Read byte from graphics memory (using interlaced addressing)
    pub fn read_byte(&self, offset: usize) -> u8 {
        // Convert from interlaced CGA address to linear offset
        let linear_offset = self.interlaced_to_linear(offset);
        if linear_offset >= self.data.len() {
            return 0;
        }
        self.data[linear_offset]
    }

    /// Write byte to graphics memory (using interlaced addressing)
    pub fn write_byte(&mut self, offset: usize, value: u8) {
        // Debug logging for graphics writes
        if value != 0 {
            let bytes_per_line = self.width * (self.bits_per_pixel as usize) / 8;
            let (y, x_byte) = if offset < 0x2000 {
                let line_in_bank = offset / bytes_per_line;
                (line_in_bank * 2, offset % bytes_per_line)
            } else {
                let offset_in_bank = offset - 0x2000;
                let line_in_bank = offset_in_bank / bytes_per_line;
                (line_in_bank * 2 + 1, offset_in_bank % bytes_per_line)
            };
            let pixels_per_byte = 8 / self.bits_per_pixel as usize;
            let x = x_byte * pixels_per_byte;
            log::debug!(
                "Graphics write: offset=0x{:04X} (x={}, y={}), value=0x{:02X} ({}bpp)",
                offset,
                x,
                y,
                value,
                self.bits_per_pixel
            );
        }

        // Convert from interlaced CGA address to linear offset
        let linear_offset = self.interlaced_to_linear(offset);
        if linear_offset >= self.data.len() {
            return;
        }

        self.data[linear_offset] = value;
    }

    /// Get pixel data as linear buffer (for rendering)
    pub fn get_pixels(&self) -> &[u8] {
        &self.data
    }

    /// Scroll window up by `lines` text rows (8 scan lines per row).
    /// `lines == 0` clears the entire window. Cleared rows are filled with 0 (black).
    /// Coordinates are in text cells (cols/rows), not pixels.
    pub fn scroll_up_window(
        &mut self,
        lines: usize,
        top: usize,
        left: usize,
        bottom: usize,
        right: usize,
    ) {
        const PIXELS_PER_ROW: usize = 8;
        let bytes_per_scan_line = self.width * self.bits_per_pixel as usize / 8;
        let bytes_per_char_col = self.bits_per_pixel as usize; // 2 for 320x200, 1 for 640x200
        let cx_start = left * bytes_per_char_col;
        let cx_end = (right + 1) * bytes_per_char_col;
        let scroll_lines = if lines == 0 { bottom - top + 1 } else { lines };

        for row in top..=bottom {
            let src_row = row + scroll_lines;
            for py in 0..PIXELS_PER_ROW {
                let dst_y = row * PIXELS_PER_ROW + py;
                let dst_base = dst_y * bytes_per_scan_line;
                if src_row <= bottom {
                    let src_y = src_row * PIXELS_PER_ROW + py;
                    let src_base = src_y * bytes_per_scan_line;
                    self.data
                        .copy_within(src_base + cx_start..src_base + cx_end, dst_base + cx_start);
                } else {
                    self.data[dst_base + cx_start..dst_base + cx_end].fill(0);
                }
            }
        }
    }

    /// Scroll window down by `lines` text rows (8 scan lines per row).
    /// `lines == 0` clears the entire window. Cleared rows are filled with 0 (black).
    pub fn scroll_down_window(
        &mut self,
        lines: usize,
        top: usize,
        left: usize,
        bottom: usize,
        right: usize,
    ) {
        const PIXELS_PER_ROW: usize = 8;
        let bytes_per_scan_line = self.width * self.bits_per_pixel as usize / 8;
        let bytes_per_char_col = self.bits_per_pixel as usize;
        let cx_start = left * bytes_per_char_col;
        let cx_end = (right + 1) * bytes_per_char_col;
        let scroll_lines = if lines == 0 { bottom - top + 1 } else { lines };

        // Iterate bottom-to-top so reads are always ahead of writes
        for row in (top..=bottom).rev() {
            let dst_base_row = row;
            let src_row = if row >= top + scroll_lines {
                row - scroll_lines
            } else {
                usize::MAX // sentinel: clear
            };
            for py in 0..PIXELS_PER_ROW {
                let dst_y = dst_base_row * PIXELS_PER_ROW + py;
                let dst_base = dst_y * bytes_per_scan_line;
                if src_row != usize::MAX && src_row >= top {
                    let src_y = src_row * PIXELS_PER_ROW + py;
                    let src_base = src_y * bytes_per_scan_line;
                    self.data
                        .copy_within(src_base + cx_start..src_base + cx_end, dst_base + cx_start);
                } else {
                    self.data[dst_base + cx_start..dst_base + cx_end].fill(0);
                }
            }
        }
    }
}

/// CGA color palette state
#[derive(Debug, Clone, Copy)]
pub struct CgaPalette {
    /// Background color (4 bits, 16 colors)
    pub background: u8,
    /// Palette select (0 or 1)
    pub palette_id: u8,
    /// Intensity/bright mode enabled
    pub intensity: bool,
}

impl CgaPalette {
    pub fn new() -> Self {
        Self {
            background: 0,
            palette_id: 0,
            intensity: false,
        }
    }

    /// Get the 4 colors for current palette
    /// Returns [background, color1, color2, color3]
    pub fn get_colors(&self) -> [u8; 4] {
        let bg = self.background;

        if self.palette_id == 0 {
            // Palette 0 (bit 5 = 0): Green, Red, Brown
            if self.intensity {
                [bg, colors::LIGHT_GREEN, colors::LIGHT_RED, colors::YELLOW]
            } else {
                // Use actual CGA hardware colors for accuracy
                [bg, colors::GREEN, colors::RED, colors::BROWN]
            }
        } else {
            // Palette 1 (bit 5 = 1): Cyan, Magenta, Light Gray/White
            if self.intensity {
                [bg, colors::LIGHT_CYAN, colors::LIGHT_MAGENTA, colors::WHITE]
            } else {
                // Use actual CGA hardware color (Light Gray)
                // On period monitors this appeared bright/white-ish
                [bg, colors::CYAN, colors::MAGENTA, colors::LIGHT_GRAY]
            }
        }
    }

    /// Parse from CGA Color Select Register (port 0x3D9)
    pub fn from_register(value: u8) -> Self {
        Self {
            background: value & 0x0F,
            palette_id: (value >> 5) & 0x01,
            intensity: (value & 0x10) != 0,
        }
    }

    /// Convert to Color Select Register value
    pub fn to_register(&self) -> u8 {
        let mut value = self.background & 0x0F;
        if self.intensity {
            value |= 0x10;
        }
        value |= (self.palette_id & 0x01) << 5;
        value
    }
}

impl Default for CgaPalette {
    fn default() -> Self {
        Self::new()
    }
}
