use crate::{Bus, cpu::Cpu};

impl Cpu {
    /// INT 10h, AH=0Dh - Read Graphics Pixel
    /// Input:
    ///   BH = page number (0 for graphics modes, ignored)
    ///   CX = column (0-319 or 0-639)
    ///   DX = row (0-199)
    /// Output:
    ///   AL = pixel color value
    pub(super) fn int10_read_pixel(&mut self, bus: &mut Bus) {
        let col = self.cx as usize;
        let row = self.dx as usize;

        let color = match bus.video().get_mode_type() {
            crate::video::VideoMode::Graphics320x200 => {
                if col >= 320 || row >= 200 {
                    0
                } else {
                    let pixels_per_byte = 4;
                    let bytes_per_line = 80;
                    let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                    let pixel_in_byte = col % pixels_per_byte;

                    let byte_val = bus.video().read_byte(byte_offset);
                    let shift = 6 - (pixel_in_byte * 2);
                    (byte_val >> shift) & 0x03
                }
            }
            crate::video::VideoMode::Graphics640x200 => {
                if col >= 640 || row >= 200 {
                    0
                } else {
                    let pixels_per_byte = 8;
                    let bytes_per_line = 80;
                    let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                    let pixel_in_byte = col % pixels_per_byte;

                    let byte_val = bus.video().read_byte(byte_offset);
                    let bit_mask = 0x80 >> pixel_in_byte;
                    if (byte_val & bit_mask) != 0 { 1 } else { 0 }
                }
            }
            crate::video::VideoMode::Text { .. } => 0,
            crate::video::VideoMode::Graphics320x200x16 => {
                // EGA 320x200 16-color: read pixel from per-plane vram offsets
                if col >= 320 || row >= 200 {
                    0
                } else {
                    let byte_offset = row * 40 + col / 8;
                    let bit = 7 - (col % 8);
                    let mut color = 0u8;
                    for plane in 0..4usize {
                        let byte_val = bus.video().read_byte(plane * 8000 + byte_offset);
                        if (byte_val >> bit) & 1 != 0 {
                            color |= 1 << plane;
                        }
                    }
                    color
                }
            }
            crate::video::VideoMode::Graphics320x200x256 => {
                // VGA mode 13h: 1 byte per pixel, linear framebuffer
                if col >= 320 || row >= 200 {
                    0
                } else {
                    bus.video().read_byte_vga(row * 320 + col)
                }
            }
        };

        self.ax = (self.ax & 0xFF00) | (color as u16);
    }
}
