// MIGRATED  //! CP437 font renderer using embedded BIOS font data
// MIGRATED  //!
// MIGRATED  //! This module provides access to both 8x16 VGA and 8x8 CGA fonts for rendering
// MIGRATED  //! CP437 (DOS) characters to pixels.
// MIGRATED  
// MIGRATED  pub const CHAR_WIDTH_8: usize = 8;
// MIGRATED  pub const CHAR_HEIGHT_16: usize = 16;
// MIGRATED  pub const CHAR_HEIGHT_8: usize = 8;
// MIGRATED  
// MIGRATED  // Default to 8x16 VGA font
// MIGRATED  pub const CHAR_WIDTH: usize = CHAR_WIDTH_8;
// MIGRATED  pub const CHAR_HEIGHT: usize = CHAR_HEIGHT_16;
// MIGRATED  
// MIGRATED  /// Standard VGA BIOS 8x16 font data (256 characters × 16 bytes each)
// MIGRATED  /// This is the IBM VGA ROM font, public domain
// MIGRATED  /// Each character is 16 bytes, one byte per row, MSB is leftmost pixel
// MIGRATED  const VGA_FONT_8X16: &[u8] = include_bytes!("IBM_VGA_8x16.bin");
// MIGRATED  
// MIGRATED  /// IBM PC/XT CGA BIOS 8x8 font data (256 characters × 8 bytes each)
// MIGRATED  /// This is the IBM PC Version 3 CGA ROM font
// MIGRATED  /// Each character is 8 bytes, one byte per row, MSB is leftmost pixel
// MIGRATED  const CGA_FONT_8X8: &[u8] = include_bytes!("IBM_VGA_8x8.bin");
// MIGRATED  
// MIGRATED  /// CP437 font wrapper around embedded BIOS fonts
// MIGRATED  pub struct Cp437Font {
// MIGRATED      vga_font_data: &'static [u8],
// MIGRATED      cga_font_data: &'static [u8],
// MIGRATED  }
// MIGRATED  
// MIGRATED  impl Cp437Font {
// MIGRATED      /// Create a new CP437 font using the embedded fonts
// MIGRATED      pub fn new() -> Self {
// MIGRATED          Self {
// MIGRATED              vga_font_data: VGA_FONT_8X16,
// MIGRATED              cga_font_data: CGA_FONT_8X8,
// MIGRATED          }
// MIGRATED      }
// MIGRATED  
// MIGRATED      /// Get the glyph data for a character from the 8x16 VGA font
// MIGRATED      /// Returns 16 bytes, each byte represents one row of 8 pixels
// MIGRATED      /// Bit 7 = leftmost pixel, Bit 0 = rightmost pixel
// MIGRATED      pub fn get_glyph_16(&self, character: u8) -> &[u8] {
// MIGRATED          let offset = (character as usize) * CHAR_HEIGHT_16;
// MIGRATED          &self.vga_font_data[offset..offset + CHAR_HEIGHT_16]
// MIGRATED      }
// MIGRATED  
// MIGRATED      /// Get the glyph data for a character from the 8x8 CGA font
// MIGRATED      /// Returns 8 bytes, each byte represents one row of 8 pixels
// MIGRATED      /// Bit 7 = leftmost pixel, Bit 0 = rightmost pixel
// MIGRATED      pub fn get_glyph_8(&self, character: u8) -> &[u8] {
// MIGRATED          let offset = (character as usize) * CHAR_HEIGHT_8;
// MIGRATED          &self.cga_font_data[offset..offset + CHAR_HEIGHT_8]
// MIGRATED      }
// MIGRATED  
// MIGRATED      /// Get the glyph data for a character (legacy method, uses 8x16)
// MIGRATED      /// Returns 16 bytes, each byte represents one row of 8 pixels
// MIGRATED      /// Bit 7 = leftmost pixel, Bit 0 = rightmost pixel
// MIGRATED      pub fn get_glyph(&self, character: u8) -> &[u8] {
// MIGRATED          self.get_glyph_16(character)
// MIGRATED      }
// MIGRATED  }
// MIGRATED  
// MIGRATED  impl Default for Cp437Font {
// MIGRATED      fn default() -> Self {
// MIGRATED          Self::new()
// MIGRATED      }
// MIGRATED  }
// MIGRATED  
// MIGRATED  #[cfg(test)]
// MIGRATED  mod tests {
// MIGRATED      use super::*;
// MIGRATED  
// MIGRATED      #[test]
// MIGRATED      fn test_vga_font_glyph_size() {
// MIGRATED          let font = Cp437Font::new();
// MIGRATED          let glyph = font.get_glyph_16(b'A');
// MIGRATED          assert_eq!(glyph.len(), CHAR_HEIGHT_16);
// MIGRATED      }
// MIGRATED  
// MIGRATED      #[test]
// MIGRATED      fn test_cga_font_glyph_size() {
// MIGRATED          let font = Cp437Font::new();
// MIGRATED          let glyph = font.get_glyph_8(b'A');
// MIGRATED          assert_eq!(glyph.len(), CHAR_HEIGHT_8);
// MIGRATED      }
// MIGRATED  
// MIGRATED      #[test]
// MIGRATED      fn test_all_characters_accessible() {
// MIGRATED          let font = Cp437Font::new();
// MIGRATED          // Test all 256 CP437 characters in both fonts
// MIGRATED          for ch in 0..=255u8 {
// MIGRATED              let glyph_16 = font.get_glyph_16(ch);
// MIGRATED              assert_eq!(glyph_16.len(), CHAR_HEIGHT_16);
// MIGRATED  
// MIGRATED              let glyph_8 = font.get_glyph_8(ch);
// MIGRATED              assert_eq!(glyph_8.len(), CHAR_HEIGHT_8);
// MIGRATED          }
// MIGRATED      }
// MIGRATED  
// MIGRATED      #[test]
// MIGRATED      fn test_font_data_size() {
// MIGRATED          assert_eq!(VGA_FONT_8X16.len(), 256 * CHAR_HEIGHT_16);
// MIGRATED          assert_eq!(CGA_FONT_8X8.len(), 256 * CHAR_HEIGHT_8);
// MIGRATED      }
// MIGRATED  }
// MIGRATED  