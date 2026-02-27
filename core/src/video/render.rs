//! Shared rendering functions for text and graphics modes.
//!
//! These functions write to RGBA buffers (640x400) and are used by both
//! the native GUI (pixels crate) and WASM (HTML5 Canvas) renderers.

use crate::font::{CHAR_HEIGHT, CHAR_WIDTH, Cp437Font};
use crate::palette::TextModePalette;
use crate::video::CursorPosition;
use crate::video::text::TextCell;

// MIGRATED  /// Convert a 6-bit VGA DAC value (0-63) to 8-bit RGB (0-255).
// MIGRATED  #[inline]
// MIGRATED  fn dac_to_8bit(val: u8) -> u8 {
// MIGRATED      let v = val & 0x3F;
// MIGRATED      (v << 2) | (v >> 4)
// MIGRATED  }

// MIGRATED  /// Render a single text cell to an RGBA buffer.
// MIGRATED  ///
// MIGRATED  /// Uses VGA DAC palette for foreground/background colors. The output buffer
// MIGRATED  /// must be at least `stride * (row * CHAR_HEIGHT + CHAR_HEIGHT) * 4` bytes.
// MIGRATED  ///
// MIGRATED  /// # Arguments
// MIGRATED  /// * `font` - CP437 font for glyph lookup
// MIGRATED  /// * `row` - Character row position (0-based)
// MIGRATED  /// * `col` - Character column position (0-based)
// MIGRATED  /// * `cell` - The text cell (character + attribute)
// MIGRATED  /// * `vga_dac_palette` - 256-entry VGA DAC palette (6-bit RGB per component)
// MIGRATED  /// * `stride` - Output buffer width in pixels (e.g., 640)
// MIGRATED  /// * `output` - RGBA buffer to write to
// MIGRATED  pub fn render_text_cell(
// MIGRATED      font: &Cp437Font,
// MIGRATED      row: usize,
// MIGRATED      col: usize,
// MIGRATED      cell: &TextCell,
// MIGRATED      vga_dac_palette: &[[u8; 3]; 256],
// MIGRATED      stride: usize,
// MIGRATED      output: &mut [u8],
// MIGRATED  ) {
// MIGRATED      let glyph = font.get_glyph(cell.character);
// MIGRATED      let fg_dac = vga_dac_palette[cell.attribute.foreground as usize];
// MIGRATED      let bg_dac = vga_dac_palette[cell.attribute.background as usize];
// MIGRATED      let fg_color = [
// MIGRATED          dac_to_8bit(fg_dac[0]),
// MIGRATED          dac_to_8bit(fg_dac[1]),
// MIGRATED          dac_to_8bit(fg_dac[2]),
// MIGRATED      ];
// MIGRATED      let bg_color = [
// MIGRATED          dac_to_8bit(bg_dac[0]),
// MIGRATED          dac_to_8bit(bg_dac[1]),
// MIGRATED          dac_to_8bit(bg_dac[2]),
// MIGRATED      ];
// MIGRATED  
// MIGRATED      let char_x = col * CHAR_WIDTH;
// MIGRATED      let char_y = row * CHAR_HEIGHT;
// MIGRATED  
// MIGRATED      for (glyph_row, &glyph_byte) in glyph.iter().enumerate() {
// MIGRATED          let pixel_y = char_y + glyph_row;
// MIGRATED  
// MIGRATED          for bit in 0..8 {
// MIGRATED              let pixel_x = char_x + bit;
// MIGRATED              let is_fg = (glyph_byte & (0x80 >> bit)) != 0;
// MIGRATED              let color = if is_fg { fg_color } else { bg_color };
// MIGRATED  
// MIGRATED              let offset = (pixel_y * stride + pixel_x) * 4;
// MIGRATED              output[offset] = color[0];
// MIGRATED              output[offset + 1] = color[1];
// MIGRATED              output[offset + 2] = color[2];
// MIGRATED              output[offset + 3] = 0xFF;
// MIGRATED          }
// MIGRATED      }
// MIGRATED  }

/// Render a text cursor (white underline) at the given position.
///
/// Draws a white block in the bottom 2 pixel rows of the character cell.
///
/// # Arguments
/// * `position` - Cursor row/col in character coordinates
/// * `stride` - Output buffer width in pixels (e.g., 640)
/// * `output` - RGBA buffer to write to
pub fn render_cursor(position: &CursorPosition, stride: usize, output: &mut [u8]) {
    let char_x = position.col * CHAR_WIDTH;
    let char_y = position.row * CHAR_HEIGHT;

    for row_offset in (CHAR_HEIGHT - 2)..CHAR_HEIGHT {
        let pixel_y = char_y + row_offset;

        for col_offset in 0..CHAR_WIDTH {
            let pixel_x = char_x + col_offset;
            let offset = (pixel_y * stride + pixel_x) * 4;

            output[offset] = 0xFF;
            output[offset + 1] = 0xFF;
            output[offset + 2] = 0xFF;
            output[offset + 3] = 0xFF;
        }
    }
}

/// Render CGA 320x200 4-color graphics to a 640x400 RGBA buffer.
///
/// Each pixel is 2 bits (4 pixels per byte). Pixel values are mapped through
/// the AC palette registers (`color_map`) to VGA DAC indices, then looked up
/// in `vga_dac_palette` for final 6-bit RGB. Output is scaled 2x2.
///
/// # Arguments
/// * `pixel_data` - 16000 bytes (80 bytes/row * 200 rows), 2bpp packed
/// * `color_map` - 4 VGA DAC indices from Attribute Controller registers
/// * `vga_dac_palette` - 256-entry VGA DAC palette (6-bit RGB per component)
/// * `output` - RGBA buffer, must be at least 640*400*4 = 1,024,000 bytes
pub fn render_cga_320x200(
    pixel_data: &[u8],
    color_map: &[u8; 4],
    vga_dac_palette: &[[u8; 3]; 256],
    output: &mut [u8],
) {
    const WIDTH: usize = 640;
    const SCALE: usize = 2;

    for y in 0..200 {
        for x in 0..320 {
            let byte_offset = y * 80 + x / 4;
            let pixel_in_byte = x % 4;
            let byte_val = pixel_data[byte_offset];
            let shift = 6 - (pixel_in_byte * 2);
            let color_index = ((byte_val >> shift) & 0x03) as usize;

            let dac_index = color_map[color_index] as usize;
            let dac = vga_dac_palette[dac_index];
            let rgb = [
                dac_to_8bit(dac[0]),
                dac_to_8bit(dac[1]),
                dac_to_8bit(dac[2]),
            ];

            for dy in 0..SCALE {
                for dx in 0..SCALE {
                    let screen_x = x * SCALE + dx;
                    let screen_y = y * SCALE + dy;
                    let offset = (screen_y * WIDTH + screen_x) * 4;

                    output[offset] = rgb[0];
                    output[offset + 1] = rgb[1];
                    output[offset + 2] = rgb[2];
                    output[offset + 3] = 0xFF;
                }
            }
        }
    }
}

/// Render CGA 640x200 monochrome graphics to a 640x400 RGBA buffer.
///
/// Each pixel is 1 bit (8 pixels per byte). Output is scaled 1x2 (doubled vertically).
///
/// # Arguments
/// * `pixel_data` - 16000 bytes (80 bytes/row * 200 rows), 1bpp packed
/// * `fg_color` - Foreground EGA color index (0-15)
/// * `bg_color` - Background EGA color index (0-15)
/// * `output` - RGBA buffer, must be at least 640*400*4 = 1,024,000 bytes
pub fn render_cga_640x200_bw(pixel_data: &[u8], fg_color: u8, bg_color: u8, output: &mut [u8]) {
    const WIDTH: usize = 640;

    let fg_rgb = TextModePalette::get_color(fg_color);
    let bg_rgb = TextModePalette::get_color(bg_color);

    for y in 0..200 {
        for x in 0..640 {
            let byte_val = pixel_data[y * 80 + x / 8];
            let bit_mask = 0x80 >> (x % 8);
            let rgb = if (byte_val & bit_mask) != 0 {
                fg_rgb
            } else {
                bg_rgb
            };

            for dy in 0..2 {
                let screen_y = y * 2 + dy;
                let offset = (screen_y * WIDTH + x) * 4;
                output[offset] = rgb[0];
                output[offset + 1] = rgb[1];
                output[offset + 2] = rgb[2];
                output[offset + 3] = 0xFF;
            }
        }
    }
}

/// Render VGA 320x200 256-color graphics (mode 0x13) to a 640x400 RGBA buffer.
///
/// Each byte in `pixel_data` is a direct color index (0-255) into `vga_dac_palette`.
/// Output is scaled 2x2 to produce a 640x400 display.
///
/// # Arguments
/// * `pixel_data` - 64000 bytes (320 * 200), one byte per pixel (0-255)
/// * `vga_dac_palette` - 256-entry VGA DAC palette (6-bit RGB per component)
/// * `output` - RGBA buffer, must be at least 640*400*4 = 1,024,000 bytes
pub fn render_vga_320x200x256(
    pixel_data: &[u8],
    vga_dac_palette: &[[u8; 3]; 256],
    output: &mut [u8],
) {
    const WIDTH: usize = 640;
    const SCALE: usize = 2;

    for y in 0..200 {
        for x in 0..320 {
            let color_index = pixel_data[y * 320 + x] as usize;
            let dac = vga_dac_palette[color_index];
            let r = dac_to_8bit(dac[0]);
            let g = dac_to_8bit(dac[1]);
            let b = dac_to_8bit(dac[2]);

            for dy in 0..SCALE {
                for dx in 0..SCALE {
                    let screen_x = x * SCALE + dx;
                    let screen_y = y * SCALE + dy;
                    let offset = (screen_y * WIDTH + screen_x) * 4;
                    output[offset] = r;
                    output[offset + 1] = g;
                    output[offset + 2] = b;
                    output[offset + 3] = 0xFF;
                }
            }
        }
    }
}

/// Render EGA 320x200 16-color graphics to a 640x400 RGBA buffer.
///
/// Each byte in `pixel_data` is a color index (0-15) already remapped
/// through the AC palette. Looked up in `vga_dac_palette` for final
/// 6-bit RGB. Output is scaled 2x2.
///
/// # Arguments
/// * `pixel_data` - 64000 bytes (320 * 200), one byte per pixel (0-15)
/// * `vga_dac_palette` - 256-entry VGA DAC palette (6-bit RGB per component)
/// * `output` - RGBA buffer, must be at least 640*400*4 = 1,024,000 bytes
pub fn render_ega_320x200x16(
    pixel_data: &[u8],
    vga_dac_palette: &[[u8; 3]; 256],
    output: &mut [u8],
) {
    const WIDTH: usize = 640;
    const SCALE: usize = 2;

    for y in 0..200 {
        for x in 0..320 {
            let color_index = pixel_data[y * 320 + x] as usize;
            let dac = vga_dac_palette[color_index];
            let r = dac_to_8bit(dac[0]);
            let g = dac_to_8bit(dac[1]);
            let b = dac_to_8bit(dac[2]);

            for dy in 0..SCALE {
                for dx in 0..SCALE {
                    let screen_x = x * SCALE + dx;
                    let screen_y = y * SCALE + dy;
                    let offset = (screen_y * WIDTH + screen_x) * 4;
                    output[offset] = r;
                    output[offset + 1] = g;
                    output[offset + 2] = b;
                    output[offset + 3] = 0xFF;
                }
            }
        }
    }
}
