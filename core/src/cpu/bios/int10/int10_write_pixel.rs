use crate::{Bus, cpu::Cpu};

impl Cpu {
    /// INT 10h, AH=0Ch - Write Graphics Pixel
    /// Input:
    ///   AL = pixel color value (0-3 for 320x200, 0-1 for 640x200)
    ///   BH = page number (0 for graphics modes, ignored)
    ///   CX = column (0-319 or 0-639)
    ///   DX = row (0-199)
    /// Output: None
    pub(super) fn int10_write_pixel(&mut self, bus: &mut Bus) {
        let color = (self.ax & 0xFF) as u8; // AL
        let col = self.cx as usize;
        let row = self.dx as usize;

        match bus.video().get_mode_type() {
            crate::video::VideoMode::Graphics320x200 => {
                if col >= 320 || row >= 200 {
                    return;
                }
                // Calculate byte offset (4 pixels per byte)
                let pixels_per_byte = 4;
                let bytes_per_line = 320 / pixels_per_byte; // 80
                let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                let pixel_in_byte = col % pixels_per_byte;

                // Read-modify-write
                let mut byte_val = bus.video().read_byte(byte_offset);
                let shift = 6 - (pixel_in_byte * 2); // MSB first
                byte_val &= !(0x03 << shift); // Clear 2 bits
                byte_val |= (color & 0x03) << shift; // Set 2 bits
                bus.video_mut().write_byte(byte_offset, byte_val);
            }
            crate::video::VideoMode::Graphics640x200 => {
                if col >= 640 || row >= 200 {
                    return;
                }
                // Calculate byte offset (8 pixels per byte)
                let pixels_per_byte = 8;
                let bytes_per_line = 640 / pixels_per_byte; // 80
                let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                let pixel_in_byte = col % pixels_per_byte;

                // Read-modify-write
                let mut byte_val = bus.video().read_byte(byte_offset);
                let bit_mask = 0x80 >> pixel_in_byte; // MSB first
                if (color & 0x01) != 0 {
                    byte_val |= bit_mask; // Set bit
                } else {
                    byte_val &= !bit_mask; // Clear bit
                }
                bus.video_mut().write_byte(byte_offset, byte_val);
            }
            crate::video::VideoMode::Text { .. } => {
                // Ignore write pixel in text mode
            }
            crate::video::VideoMode::Graphics320x200x16 => {
                // EGA 320x200 16-color: write pixel via per-plane vram offsets
                if col < 320 && row < 200 {
                    let byte_offset = row * 40 + col / 8;
                    let bit = 7 - (col % 8);
                    for plane in 0..4usize {
                        let plane_bit = (color as usize >> plane) & 1;
                        let vram_offset = plane * 8000 + byte_offset;
                        let mut byte_val = bus.video().read_byte(vram_offset);
                        if plane_bit != 0 {
                            byte_val |= 1 << bit;
                        } else {
                            byte_val &= !(1 << bit);
                        }
                        bus.video_mut().write_byte(vram_offset, byte_val);
                    }
                }
            }
            crate::video::VideoMode::Graphics320x200x256 => {
                // VGA mode 13h: 1 byte per pixel, linear framebuffer
                if col < 320 && row < 200 {
                    bus.video_mut().write_byte_vga(row * 320 + col, color);
                }
            }
        }
    }
}
