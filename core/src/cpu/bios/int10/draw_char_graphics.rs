use crate::cpu::Cpu;
use crate::cpu::bios::int10::GraphicsDrawMode;
use crate::{Bus, Cp437Font};

impl Cpu {
    /// Draw a character in graphics mode using font data
    pub(super) fn draw_char_graphics(
        &self,
        bus: &mut Bus,
        character: u8,
        row: usize,
        col: usize,
        fg_color: u8,
        mode: GraphicsDrawMode,
    ) {
        match bus.video().get_mode_type() {
            crate::video::VideoMode::Graphics320x200 | crate::video::VideoMode::Graphics640x200 => {
                self.draw_char_graphics_cga(bus, character, row, col, fg_color, mode);
            }
            crate::video::VideoMode::Graphics320x200x16 => {
                self.draw_char_graphics_ega(bus, character, row, col, fg_color, mode);
            }
            crate::video::VideoMode::Graphics320x200x256 => {
                self.draw_char_graphics_vga(bus, character, row, col, fg_color, mode);
            }
            _ => {
                // Other modes not supported yet
            }
        }
    }

    fn draw_char_graphics_cga(
        &self,
        bus: &mut Bus,
        character: u8,
        row: usize,
        col: usize,
        fg_color: u8,
        mode: GraphicsDrawMode,
    ) {
        let font = Cp437Font::new();

        // CGA graphics mode: use native 8x8 CGA font
        // In XorInverted mode, strip bit 7 from character to get base glyph
        // (e.g., 0x80 -> 0x00, then invert to get solid block)
        let char_code = if matches!(mode, GraphicsDrawMode::XorInverted) {
            character & 0x7F
        } else {
            character
        };
        let glyph = font.get_glyph_8(char_code);
        let char_height = 8;
        let char_width = 8;

        // Get screen dimensions based on mode
        let screen_width = match bus.video().get_mode_type() {
            crate::video::VideoMode::Graphics320x200 => 320,
            crate::video::VideoMode::Graphics640x200 => 640,
            _ => 320, // fallback
        };

        // Calculate pixel position
        let start_x = col * char_width;
        let start_y = row * char_height;

        log::trace!(
            "Drawing char 0x{:02X}->0x{:02X} at row={} col={} (pixel {},{}) fg={} mode={:?}",
            character,
            char_code,
            row,
            col,
            start_x,
            start_y,
            fg_color,
            mode
        );

        // Draw each row of the character
        for (py, &glyph_byte) in glyph.iter().enumerate() {
            if start_y + py >= 200 {
                break; // Don't draw past bottom of screen
            }

            // In XorInverted mode, invert the glyph (swap foreground/background pixels)
            // This makes character 0x80 (blank in CGA font) become a solid block for inverse effect
            let final_glyph_byte = if matches!(mode, GraphicsDrawMode::XorInverted) {
                !glyph_byte // Invert all bits
            } else {
                glyph_byte
            };

            // Draw each pixel in the row
            for px in 0..char_width {
                if start_x + px >= screen_width {
                    break; // Don't draw past right edge
                }

                let bit = (final_glyph_byte >> (7 - px)) & 1;
                if bit != 0 {
                    // Foreground pixel
                    match mode {
                        GraphicsDrawMode::Xor | GraphicsDrawMode::XorInverted => {
                            // XOR mode: read current pixel and XOR with fg_color
                            let current = self.get_pixel_cga(bus, start_x + px, start_y + py);
                            let new_color = current ^ fg_color;
                            self.set_pixel_cga(bus, start_x + px, start_y + py, new_color);
                        }
                        GraphicsDrawMode::Transparent | GraphicsDrawMode::Opaque => {
                            // Normal mode: use fg_color
                            self.set_pixel_cga(bus, start_x + px, start_y + py, fg_color);
                        }
                    }
                } else if matches!(mode, GraphicsDrawMode::Opaque) {
                    // Background pixel - only draw in opaque mode (bg=0)
                    self.set_pixel_cga(bus, start_x + px, start_y + py, 0);
                }
                // Otherwise leave background transparent (don't draw)
            }
        }
    }

    fn draw_char_graphics_ega(
        &self,
        bus: &mut Bus,
        character: u8,
        row: usize,
        col: usize,
        fg_color: u8,
        mode: GraphicsDrawMode,
    ) {
        let font = Cp437Font::new();

        // EGA 320x200 16-color planar mode
        let char_code = if matches!(mode, GraphicsDrawMode::XorInverted) {
            character & 0x7F
        } else {
            character
        };
        let glyph = font.get_glyph_8(char_code);
        let char_height = 8;
        let char_width = 8;

        let start_x = col * char_width;
        let start_y = row * char_height;

        for (py, &glyph_byte) in glyph.iter().enumerate() {
            let y = start_y + py;
            if y >= 200 {
                break;
            }

            let final_glyph_byte = if matches!(mode, GraphicsDrawMode::XorInverted) {
                !glyph_byte
            } else {
                glyph_byte
            };

            // For EGA, work byte-at-a-time per plane for efficiency
            // All 8 pixels of a glyph row fit in one EGA byte if char is byte-aligned
            let byte_offset = y * 40 + start_x / 8;
            let bit_shift = start_x % 8;

            if bit_shift == 0 {
                // Byte-aligned: fast path
                for plane in 0..4usize {
                    let plane_bit = (fg_color >> plane) & 1;
                    let vram_offset = plane * 8000 + byte_offset;
                    let current = bus.video().read_byte(vram_offset);

                    let new_val = match mode {
                        GraphicsDrawMode::Opaque => {
                            // Foreground where glyph=1, background (0) where glyph=0
                            if plane_bit != 0 { final_glyph_byte } else { 0 }
                        }
                        GraphicsDrawMode::Transparent => {
                            // Only set foreground pixels, leave background untouched
                            if plane_bit != 0 {
                                current | final_glyph_byte
                            } else {
                                current & !final_glyph_byte
                            }
                        }
                        GraphicsDrawMode::Xor | GraphicsDrawMode::XorInverted => {
                            if plane_bit != 0 {
                                current ^ final_glyph_byte
                            } else {
                                current
                            }
                        }
                    };
                    bus.video_mut().write_byte(vram_offset, new_val);
                }
            } else {
                // Not byte-aligned: spans two EGA bytes
                let left_mask = final_glyph_byte >> bit_shift;
                let right_mask = final_glyph_byte << (8 - bit_shift);

                for plane in 0..4usize {
                    let plane_bit = (fg_color >> plane) & 1;
                    let vram_offset = plane * 8000 + byte_offset;

                    // Left byte
                    let cur_left = bus.video().read_byte(vram_offset);
                    let new_left = match mode {
                        GraphicsDrawMode::Opaque => {
                            if plane_bit != 0 {
                                (cur_left & !left_mask) | left_mask
                            } else {
                                cur_left & !left_mask
                            }
                        }
                        GraphicsDrawMode::Transparent => {
                            if plane_bit != 0 {
                                cur_left | left_mask
                            } else {
                                cur_left & !left_mask
                            }
                        }
                        GraphicsDrawMode::Xor | GraphicsDrawMode::XorInverted => {
                            if plane_bit != 0 {
                                cur_left ^ left_mask
                            } else {
                                cur_left
                            }
                        }
                    };
                    bus.video_mut().write_byte(vram_offset, new_left);

                    // Right byte (if any pixels spill over)
                    if right_mask != 0 && byte_offset + 1 < 8000 {
                        let vram_offset_r = plane * 8000 + byte_offset + 1;
                        let cur_right = bus.video().read_byte(vram_offset_r);
                        let new_right = match mode {
                            GraphicsDrawMode::Opaque => {
                                if plane_bit != 0 {
                                    (cur_right & !right_mask) | right_mask
                                } else {
                                    cur_right & !right_mask
                                }
                            }
                            GraphicsDrawMode::Transparent => {
                                if plane_bit != 0 {
                                    cur_right | right_mask
                                } else {
                                    cur_right & !right_mask
                                }
                            }
                            GraphicsDrawMode::Xor | GraphicsDrawMode::XorInverted => {
                                if plane_bit != 0 {
                                    cur_right ^ right_mask
                                } else {
                                    cur_right
                                }
                            }
                        };
                        bus.video_mut().write_byte(vram_offset_r, new_right);
                    }
                }
            }
        }
    }

    fn draw_char_graphics_vga(
        &self,
        bus: &mut Bus,
        character: u8,
        row: usize,
        col: usize,
        fg_color: u8,
        mode: GraphicsDrawMode,
    ) {
        let font = Cp437Font::new();

        // VGA mode 13h: 1 byte per pixel, linear framebuffer
        let char_code = if matches!(mode, GraphicsDrawMode::XorInverted) {
            character & 0x7F
        } else {
            character
        };
        let glyph = font.get_glyph_8(char_code);
        let char_height = 8;
        let char_width = 8;

        let start_x = col * char_width;
        let start_y = row * char_height;

        for (py, &glyph_byte) in glyph.iter().enumerate() {
            let y = start_y + py;
            if y >= 200 {
                break;
            }

            let final_glyph_byte = if matches!(mode, GraphicsDrawMode::XorInverted) {
                !glyph_byte
            } else {
                glyph_byte
            };

            for px in 0..char_width {
                let x = start_x + px;
                if x >= 320 {
                    break;
                }
                let offset = y * 320 + x;
                let bit = (final_glyph_byte >> (7 - px)) & 1;
                if bit != 0 {
                    match mode {
                        GraphicsDrawMode::Xor | GraphicsDrawMode::XorInverted => {
                            let current = bus.video().read_byte_vga(offset);
                            bus.video_mut().write_byte_vga(offset, current ^ fg_color);
                        }
                        GraphicsDrawMode::Transparent | GraphicsDrawMode::Opaque => {
                            bus.video_mut().write_byte_vga(offset, fg_color);
                        }
                    }
                } else if matches!(mode, GraphicsDrawMode::Opaque) {
                    bus.video_mut().write_byte_vga(offset, 0);
                }
            }
        }
    }

    /// Get a pixel value in CGA graphics mode
    /// Calculates CGA interlaced address: even lines at 0x0000+, odd lines at 0x2000+
    fn get_pixel_cga(&self, bus: &mut Bus, x: usize, y: usize) -> u8 {
        match bus.video().get_mode_type() {
            crate::video::VideoMode::Graphics320x200 => {
                // 320x200, 4 colors, 2 bits per pixel
                let byte_offset = if y.is_multiple_of(2) {
                    (y / 2) * 80 + x / 4
                } else {
                    0x2000 + ((y - 1) / 2) * 80 + x / 4
                };
                let pixel_in_byte = x % 4;
                let shift = 6 - (pixel_in_byte * 2);
                let byte_val = bus.video().read_byte(byte_offset);
                (byte_val >> shift) & 0x03
            }
            crate::video::VideoMode::Graphics640x200 => {
                // 640x200, 2 colors, 1 bit per pixel
                let byte_offset = if y.is_multiple_of(2) {
                    (y / 2) * 80 + x / 8
                } else {
                    0x2000 + ((y - 1) / 2) * 80 + x / 8
                };
                let pixel_in_byte = x % 8;
                let shift = 7 - pixel_in_byte;
                let byte_val = bus.video().read_byte(byte_offset);
                (byte_val >> shift) & 0x01
            }
            _ => 0,
        }
    }

    /// Set a pixel in CGA graphics mode
    /// Calculates CGA interlaced address: even lines at 0x0000+, odd lines at 0x2000+
    fn set_pixel_cga(&self, bus: &mut Bus, x: usize, y: usize, color: u8) {
        match bus.video().get_mode_type() {
            crate::video::VideoMode::Graphics320x200 => {
                // 320x200, 4 colors, 2 bits per pixel
                // Calculate CGA interlaced offset
                let byte_offset = if y.is_multiple_of(2) {
                    // Even line: bank 0
                    (y / 2) * 80 + x / 4
                } else {
                    // Odd line: bank 1 (offset by 0x2000)
                    0x2000 + ((y - 1) / 2) * 80 + x / 4
                };

                let pixel_in_byte = x % 4;
                let shift = 6 - (pixel_in_byte * 2);

                // Read-modify-write
                let mut byte_val = bus.video().read_byte(byte_offset);
                byte_val &= !(0x03 << shift); // Clear the 2-bit pixel
                byte_val |= (color & 0x03) << shift; // Set new color
                bus.video_mut().write_byte(byte_offset, byte_val);
            }
            crate::video::VideoMode::Graphics640x200 => {
                // 640x200, 2 colors, 1 bit per pixel
                // Calculate CGA interlaced offset
                let byte_offset = if y.is_multiple_of(2) {
                    // Even line: bank 0
                    (y / 2) * 80 + x / 8
                } else {
                    // Odd line: bank 1 (offset by 0x2000)
                    0x2000 + ((y - 1) / 2) * 80 + x / 8
                };

                let pixel_in_byte = x % 8;
                let bit_mask = 0x80 >> pixel_in_byte;

                let mut byte_val = bus.video().read_byte(byte_offset);
                if color & 1 != 0 {
                    byte_val |= bit_mask; // Set bit
                } else {
                    byte_val &= !bit_mask; // Clear bit
                }
                bus.video_mut().write_byte(byte_offset, byte_val);
            }
            _ => {
                // Not a graphics mode
            }
        }
    }
}
