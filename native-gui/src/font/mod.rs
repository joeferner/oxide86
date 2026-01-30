//! CP437 font renderer using embedded VGA BIOS font data
//!
//! This module provides access to the standard 8x16 VGA font for rendering
//! CP437 (DOS) characters to pixels.

#[allow(dead_code)]
pub const CHAR_WIDTH: usize = 8;
#[allow(dead_code)]
pub const CHAR_HEIGHT: usize = 16;

/// Standard VGA BIOS 8x16 font data (256 characters × 16 bytes each)
/// This is the IBM VGA ROM font, public domain
/// Each character is 16 bytes, one byte per row, MSB is leftmost pixel
const VGA_FONT_8X16: &[u8] = include_bytes!("IBM_VGA_8x16.bin");

/// CP437 font wrapper around embedded VGA BIOS font
#[allow(dead_code)]
pub struct Cp437Font {
    font_data: &'static [u8],
}

impl Cp437Font {
    /// Create a new CP437 font using the embedded VGA 8x16 font
    pub fn new() -> Self {
        Self {
            font_data: VGA_FONT_8X16,
        }
    }

    /// Get the glyph data for a character
    /// Returns 16 bytes, each byte represents one row of 8 pixels
    /// Bit 7 = leftmost pixel, Bit 0 = rightmost pixel
    pub fn get_glyph(&self, character: u8) -> &[u8] {
        let offset = (character as usize) * CHAR_HEIGHT;
        &self.font_data[offset..offset + CHAR_HEIGHT]
    }
}

impl Default for Cp437Font {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_glyph_size() {
        let font = Cp437Font::new();
        let glyph = font.get_glyph(b'A');
        assert_eq!(glyph.len(), CHAR_HEIGHT);
    }

    #[test]
    fn test_all_characters_accessible() {
        let font = Cp437Font::new();
        // Test all 256 CP437 characters
        for ch in 0..=255u8 {
            let glyph = font.get_glyph(ch);
            assert_eq!(glyph.len(), CHAR_HEIGHT);
        }
    }

    #[test]
    fn test_font_data_size() {
        assert_eq!(VGA_FONT_8X16.len(), 256 * CHAR_HEIGHT);
    }
}
