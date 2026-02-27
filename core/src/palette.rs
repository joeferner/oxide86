// MIGRATED  pub struct TextModePalette {}
// MIGRATED  
// MIGRATED  impl TextModePalette {
// MIGRATED      /// Get 8-bit RGB color value (0-255) for rendering
// MIGRATED      pub fn get_color(color: u8) -> [u8; 3] {
// MIGRATED          match color & 0x0F {
// MIGRATED              0 => [0x00, 0x00, 0x00],  // Black
// MIGRATED              1 => [0x00, 0x00, 0xAA],  // Blue
// MIGRATED              2 => [0x00, 0xAA, 0x00],  // Green
// MIGRATED              3 => [0x00, 0xAA, 0xAA],  // Cyan
// MIGRATED              4 => [0xAA, 0x00, 0x00],  // Red
// MIGRATED              5 => [0xAA, 0x00, 0xAA],  // Magenta
// MIGRATED              6 => [0xAA, 0x55, 0x00],  // Brown
// MIGRATED              7 => [0xAA, 0xAA, 0xAA],  // Light Gray
// MIGRATED              8 => [0x55, 0x55, 0x55],  // Dark Gray
// MIGRATED              9 => [0x55, 0x55, 0xFF],  // Light Blue
// MIGRATED              10 => [0x55, 0xFF, 0x55], // Light Green
// MIGRATED              11 => [0x55, 0xFF, 0xFF], // Light Cyan
// MIGRATED              12 => [0xFF, 0x55, 0x55], // Light Red
// MIGRATED              13 => [0xFF, 0x55, 0xFF], // Light Magenta
// MIGRATED              14 => [0xFF, 0xFF, 0x55], // Yellow
// MIGRATED              15 => [0xFF, 0xFF, 0xFF], // White
// MIGRATED              _ => [0xFF, 0xFF, 0xFF],  // Fallback to white
// MIGRATED          }
// MIGRATED      }
// MIGRATED  
// MIGRATED      /// Get 6-bit RGB color value (0-63) for VGA DAC registers
// MIGRATED      pub fn get_dac_color(color: u8) -> [u8; 3] {
// MIGRATED          match color & 0x0F {
// MIGRATED              0 => [0x00, 0x00, 0x00],  // Black
// MIGRATED              1 => [0x00, 0x00, 0x2A],  // Blue
// MIGRATED              2 => [0x00, 0x2A, 0x00],  // Green
// MIGRATED              3 => [0x00, 0x2A, 0x2A],  // Cyan
// MIGRATED              4 => [0x2A, 0x00, 0x00],  // Red
// MIGRATED              5 => [0x2A, 0x00, 0x2A],  // Magenta
// MIGRATED              6 => [0x2A, 0x15, 0x00],  // Brown
// MIGRATED              7 => [0x2A, 0x2A, 0x2A],  // Light Gray
// MIGRATED              8 => [0x15, 0x15, 0x15],  // Dark Gray
// MIGRATED              9 => [0x15, 0x15, 0x3F],  // Light Blue
// MIGRATED              10 => [0x15, 0x3F, 0x15], // Light Green
// MIGRATED              11 => [0x15, 0x3F, 0x3F], // Light Cyan
// MIGRATED              12 => [0x3F, 0x15, 0x15], // Light Red
// MIGRATED              13 => [0x3F, 0x15, 0x3F], // Light Magenta
// MIGRATED              14 => [0x3F, 0x3F, 0x15], // Yellow
// MIGRATED              15 => [0x3F, 0x3F, 0x3F], // White
// MIGRATED              _ => [0x3F, 0x3F, 0x3F],  // Fallback to white
// MIGRATED          }
// MIGRATED      }
// MIGRATED  }
// MIGRATED  