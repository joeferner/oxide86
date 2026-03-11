//! CP437 font renderer using embedded BIOS font data
//!
//! This module provides access to both 8x16 VGA and 8x8 CGA fonts for rendering
//! CP437 (DOS) characters to pixels.

pub const CHAR_WIDTH_8: usize = 8;
pub const CHAR_HEIGHT_16: usize = 16;
pub const CHAR_HEIGHT_14: usize = 14;
pub const CHAR_HEIGHT_8: usize = 8;

// Default to 8x16 VGA font
pub const CHAR_WIDTH: usize = CHAR_WIDTH_8;
pub const CHAR_HEIGHT: usize = CHAR_HEIGHT_16;

/// Standard VGA BIOS 8x16 font data (256 characters × 16 bytes each)
/// This is the IBM VGA ROM font, public domain
/// Each character is 16 bytes, one byte per row, MSB is leftmost pixel
const VGA_FONT_8X16: &[u8] = include_bytes!("IBM_VGA_8x16.bin");

/// IBM PC/XT CGA BIOS 8x8 font data (256 characters × 8 bytes each)
/// This is the IBM PC Version 3 CGA ROM font
/// Each character is 8 bytes, one byte per row, MSB is leftmost pixel
const CGA_FONT_8X8: &[u8] = include_bytes!("IBM_VGA_8x8.bin");

/// Public re-export of the raw 8x8 font bytes for BIOS ROM mapping.
pub const CGA_FONT_8X8_DATA: &[u8] = CGA_FONT_8X8;

/// CP437 font wrapper around embedded BIOS fonts
#[derive(Clone)]
pub(crate) struct Cp437Font {
    vga_font_data: &'static [u8],
    cga_font_data: &'static [u8],
}

impl Cp437Font {
    /// Create a new CP437 font using the embedded fonts
    pub(crate) fn new() -> Self {
        Self {
            vga_font_data: VGA_FONT_8X16,
            cga_font_data: CGA_FONT_8X8,
        }
    }

    /// Get the glyph data for a character from the 8x16 VGA font
    /// Returns 16 bytes, each byte represents one row of 8 pixels
    /// Bit 7 = leftmost pixel, Bit 0 = rightmost pixel
    pub(crate) fn get_glyph_16(&self, character: u8) -> &[u8] {
        let offset = (character as usize) * CHAR_HEIGHT_16;
        &self.vga_font_data[offset..offset + CHAR_HEIGHT_16]
    }

    /// Get the glyph data for a character from the 8x8 CGA font
    /// Returns 8 bytes, each byte represents one row of 8 pixels
    /// Bit 7 = leftmost pixel, Bit 0 = rightmost pixel
    pub(crate) fn get_glyph_8(&self, character: u8) -> &[u8] {
        let offset = (character as usize) * CHAR_HEIGHT_8;
        &self.cga_font_data[offset..offset + CHAR_HEIGHT_8]
    }

    /// Get the glyph data for a character (legacy method, uses 8x16)
    /// Returns 16 bytes, each byte represents one row of 8 pixels
    /// Bit 7 = leftmost pixel, Bit 0 = rightmost pixel
    pub(crate) fn get_glyph(&self, character: u8) -> &[u8] {
        self.get_glyph_16(character)
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

    #[test_log::test]
    fn test_vga_font_glyph_size() {
        let font = Cp437Font::new();
        let glyph = font.get_glyph_16(b'A');
        assert_eq!(glyph.len(), CHAR_HEIGHT_16);
    }

    #[test_log::test]
    fn test_cga_font_glyph_size() {
        let font = Cp437Font::new();
        let glyph = font.get_glyph_8(b'A');
        assert_eq!(glyph.len(), CHAR_HEIGHT_8);
    }

    #[test_log::test]
    fn test_all_characters_accessible() {
        let font = Cp437Font::new();
        // Test all 256 CP437 characters in both fonts
        for ch in 0..=255u8 {
            let glyph_16 = font.get_glyph_16(ch);
            assert_eq!(glyph_16.len(), CHAR_HEIGHT_16);

            let glyph_8 = font.get_glyph_8(ch);
            assert_eq!(glyph_8.len(), CHAR_HEIGHT_8);
        }
    }

    #[test_log::test]
    fn test_font_data_size() {
        assert_eq!(VGA_FONT_8X16.len(), 256 * CHAR_HEIGHT_16);
        assert_eq!(CGA_FONT_8X8.len(), 256 * CHAR_HEIGHT_8);
    }
}
