/// EGA planar framebuffer for mode 0x0D (320x200 16-color)
/// 4 bit planes, each 8000 bytes (320*200/8 pixels per byte)
pub struct EgaBuffer {
    /// 4 bit planes, each 8000 bytes
    planes: [[u8; 8000]; 4],
}

impl EgaBuffer {
    pub fn new() -> Self {
        Self {
            planes: [[0u8; 8000]; 4],
        }
    }

    /// Write a byte to all enabled planes (map_mask = bitmask of planes 0-3)
    pub fn write_byte(&mut self, map_mask: u8, offset: usize, value: u8) {
        if offset >= 8000 {
            return;
        }
        for plane in 0..4 {
            if map_mask & (1 << plane) != 0 {
                self.planes[plane][offset] = value;
            }
        }
    }

    /// Read a byte from the selected plane
    pub fn read_byte(&self, read_plane: u8, offset: usize) -> u8 {
        let plane = (read_plane & 3) as usize;
        if offset >= 8000 {
            return 0;
        }
        self.planes[plane][offset]
    }

    /// Scroll window up by `lines` text rows (8 scan lines per row).
    /// `lines == 0` clears the entire window. Cleared rows are filled with 0 (black).
    /// Coordinates are in text cells (40 cols x 25 rows for 320x200).
    pub fn scroll_up_window(
        &mut self,
        lines: usize,
        top: usize,
        left: usize,
        bottom: usize,
        right: usize,
    ) {
        const PIXELS_PER_ROW: usize = 8;
        const BYTES_PER_SCAN_LINE: usize = 40; // 320px / 8px per byte
        let scroll_lines = if lines == 0 { bottom - top + 1 } else { lines };

        for plane in 0..4usize {
            for row in top..=bottom {
                let src_row = row + scroll_lines;
                for py in 0..PIXELS_PER_ROW {
                    let dst_y = row * PIXELS_PER_ROW + py;
                    let dst_base = dst_y * BYTES_PER_SCAN_LINE;
                    if src_row <= bottom {
                        let src_y = src_row * PIXELS_PER_ROW + py;
                        let src_base = src_y * BYTES_PER_SCAN_LINE;
                        for cx in left..=right {
                            self.planes[plane][dst_base + cx] = self.planes[plane][src_base + cx];
                        }
                    } else {
                        for cx in left..=right {
                            self.planes[plane][dst_base + cx] = 0;
                        }
                    }
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
        const BYTES_PER_SCAN_LINE: usize = 40;
        let scroll_lines = if lines == 0 { bottom - top + 1 } else { lines };

        for plane in 0..4usize {
            for row in (top..=bottom).rev() {
                let src_row = if row >= top + scroll_lines {
                    row - scroll_lines
                } else {
                    usize::MAX
                };
                for py in 0..PIXELS_PER_ROW {
                    let dst_y = row * PIXELS_PER_ROW + py;
                    let dst_base = dst_y * BYTES_PER_SCAN_LINE;
                    if src_row != usize::MAX && src_row >= top {
                        let src_y = src_row * PIXELS_PER_ROW + py;
                        let src_base = src_y * BYTES_PER_SCAN_LINE;
                        for cx in left..=right {
                            self.planes[plane][dst_base + cx] = self.planes[plane][src_base + cx];
                        }
                    } else {
                        for cx in left..=right {
                            self.planes[plane][dst_base + cx] = 0;
                        }
                    }
                }
            }
        }
    }

    /// Compose 16-color pixel data (320*200 = 64000 bytes, each byte is a 0-15 color index)
    pub fn get_pixels(&self) -> Vec<u8> {
        let mut pixels = vec![0u8; 320 * 200];
        for y in 0..200usize {
            for x in 0..320usize {
                let byte_offset = y * 40 + x / 8;
                let bit = 7 - (x % 8);
                let color = ((self.planes[0][byte_offset] >> bit) & 1)
                    | (((self.planes[1][byte_offset] >> bit) & 1) << 1)
                    | (((self.planes[2][byte_offset] >> bit) & 1) << 2)
                    | (((self.planes[3][byte_offset] >> bit) & 1) << 3);
                pixels[y * 320 + x] = color;
            }
        }
        pixels
    }
}
