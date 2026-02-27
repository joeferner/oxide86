use std::ops::{Index, IndexMut};

use crate::{
    colors,
    video::{TEXT_MODE_COLS, TEXT_MODE_ROWS},
};

// MIGRATED  /// VGA text mode character attribute
// MIGRATED  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// MIGRATED  pub struct TextAttribute {
// MIGRATED      pub foreground: u8, // 4 bits (0-15)
// MIGRATED      pub background: u8, // 3 bits in blink mode (0-7), 4 bits in intensity mode (0-15)
// MIGRATED      pub blink: bool,    // bit 7 when blink_enabled=true; always false in intensity mode
// MIGRATED  }
// MIGRATED  
// MIGRATED  impl TextAttribute {
// MIGRATED      /// Create from attribute byte.
// MIGRATED      ///
// MIGRATED      /// When `blink_enabled` is true (default), bit 7 = character blink,
// MIGRATED      /// background uses bits 4-6 (8 colors).
// MIGRATED      /// When `blink_enabled` is false (intensity mode), bit 7 is the high bit
// MIGRATED      /// of the background color, giving 16 background colors with no blink.
// MIGRATED      pub fn from_byte(byte: u8, blink_enabled: bool) -> Self {
// MIGRATED          if blink_enabled {
// MIGRATED              Self {
// MIGRATED                  foreground: byte & 0x0F,
// MIGRATED                  background: (byte >> 4) & 0x07,
// MIGRATED                  blink: (byte & 0x80) != 0,
// MIGRATED              }
// MIGRATED          } else {
// MIGRATED              Self {
// MIGRATED                  foreground: byte & 0x0F,
// MIGRATED                  background: (byte >> 4) & 0x0F,
// MIGRATED                  blink: false,
// MIGRATED              }
// MIGRATED          }
// MIGRATED      }
// MIGRATED  
// MIGRATED      /// Convert to VGA attribute byte (always uses blink-mode encoding)
// MIGRATED      pub fn to_byte(&self) -> u8 {
// MIGRATED          let mut byte = self.foreground & 0x0F;
// MIGRATED          byte |= (self.background & 0x07) << 4;
// MIGRATED          if self.blink {
// MIGRATED              byte |= 0x80;
// MIGRATED          }
// MIGRATED          byte
// MIGRATED      }
// MIGRATED  }
// MIGRATED  
// MIGRATED  impl Default for TextAttribute {
// MIGRATED      fn default() -> Self {
// MIGRATED          Self {
// MIGRATED              foreground: colors::LIGHT_GRAY,
// MIGRATED              background: colors::BLACK,
// MIGRATED              blink: false,
// MIGRATED          }
// MIGRATED      }
// MIGRATED  }

/// A single character cell in text mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCell {
    pub character: u8,
    pub attribute: TextAttribute,
}

impl Default for TextCell {
    fn default() -> Self {
        Self {
            character: 0x20, // Space character
            attribute: TextAttribute::default(),
        }
    }
}

pub struct TextBuffer {
    buffer: [TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS],
}

impl TextBuffer {
    pub fn new() -> Self {
        Self {
            buffer: [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS],
        }
    }
}

impl TextBuffer {
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn copy_from(&mut self, other: &TextBuffer) {
        self.buffer.copy_from_slice(&other.buffer);
    }
}

impl Index<usize> for TextBuffer {
    type Output = TextCell;

    fn index(&self, idx: usize) -> &Self::Output {
        &self.buffer[idx]
    }
}

impl IndexMut<usize> for TextBuffer {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        &mut self.buffer[idx]
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}
