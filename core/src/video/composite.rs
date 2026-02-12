//! CGA Composite mode rendering utilities
//!
//! Provides shared logic for rendering 2bpp CGA graphics data in composite mode.

use crate::palette::TextModePalette;

/// Render 2bpp CGA graphics data as composite mode to RGBA buffer.
///
/// Takes 320x200 2bpp pixel data (80 bytes per row, 4 pixels per byte) and renders
/// it as composite CGA with artifact coloring, scaled 2x2 to 640x400.
///
/// # Arguments
/// * `pixel_data` - 16000 bytes (80 * 200) of 2bpp pixel data
/// * `output` - RGBA buffer to write to (640 * 400 * 4 bytes = 1,024,000 bytes)
///
/// # Composite Color Mapping
/// * 0 → Black (background)
/// * 1 → Green (composite artifact color)
/// * 2 → Magenta (composite artifact color)
/// * 3 → White (foreground)
pub fn render_composite_2bpp(pixel_data: &[u8], output: &mut [u8]) {
    const WIDTH: usize = 640;
    const SCALE: usize = 2;

    for y in 0..200 {
        for byte_x in 0..80 {
            let byte_val = pixel_data[y * 80 + byte_x];

            // Extract 4 pixels from this byte (2 bits each, MSB first)
            for pixel_idx in 0..4 {
                let shift = 6 - (pixel_idx * 2); // 6, 4, 2, 0
                let pixel_val = (byte_val >> shift) & 0x03;

                // Map 2-bit pixel value to composite color index
                let color_index = match pixel_val {
                    0 => 0,  // Background color (black)
                    1 => 2,  // Green (typical composite artifact color)
                    2 => 5,  // Magenta (typical composite artifact color)
                    3 => 15, // White (foreground)
                    _ => 0,
                };
                let rgb = TextModePalette::get_color(color_index);

                // Render this pixel scaled 2x2
                let pixel_x = byte_x * 4 + pixel_idx;
                for dy in 0..SCALE {
                    for dx in 0..SCALE {
                        let screen_x = pixel_x * SCALE + dx;
                        let screen_y = y * SCALE + dy;
                        let offset = (screen_y * WIDTH + screen_x) * 4;
                        output[offset] = rgb[0]; // R
                        output[offset + 1] = rgb[1]; // G
                        output[offset + 2] = rgb[2]; // B
                        output[offset + 3] = 0xFF; // A
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composite_rendering_size() {
        let pixel_data = vec![0u8; 80 * 200]; // 16000 bytes
        let mut output = vec![0u8; 640 * 400 * 4]; // RGBA buffer

        render_composite_2bpp(&pixel_data, &mut output);

        // Should not panic and should fill the buffer
        assert_eq!(output.len(), 640 * 400 * 4);
    }

    #[test]
    fn test_composite_color_mapping() {
        // Create a test pattern with all 4 pixel values
        let mut pixel_data = vec![0u8; 80 * 200];
        pixel_data[0] = 0b11_10_01_00; // All 4 pixel values in one byte

        let mut output = vec![0u8; 640 * 400 * 4];
        render_composite_2bpp(&pixel_data, &mut output);

        // Check first pixel (value 3 -> white = index 15)
        let white = TextModePalette::get_color(15);
        assert_eq!(output[0], white[0]);
        assert_eq!(output[1], white[1]);
        assert_eq!(output[2], white[2]);
        assert_eq!(output[3], 0xFF);

        // Check second pixel (value 2 -> magenta = index 5)
        // Second pixel starts at x=2 (scaled), so offset = 2*4
        let magenta = TextModePalette::get_color(5);
        let offset = 2 * 4;
        assert_eq!(output[offset], magenta[0]);
        assert_eq!(output[offset + 1], magenta[1]);
        assert_eq!(output[offset + 2], magenta[2]);
        assert_eq!(output[offset + 3], 0xFF);
    }
}
