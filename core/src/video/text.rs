use std::ops::{Index, IndexMut};

use crate::{
    colors,
    video::{TEXT_MODE_COLS, TEXT_MODE_ROWS},
};

/// VGA text mode character attribute
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextAttribute {
    pub foreground: u8, // 4 bits (0-15)
    pub background: u8, // 3 bits in blink mode (0-7), 4 bits in intensity mode (0-15)
    pub blink: bool,    // bit 7 when blink_enabled=true; always false in intensity mode
}

impl TextAttribute {
    /// Create from attribute byte.
    ///
    /// When `blink_enabled` is true (default), bit 7 = character blink,
    /// background uses bits 4-6 (8 colors).
    /// When `blink_enabled` is false (intensity mode), bit 7 is the high bit
    /// of the background color, giving 16 background colors with no blink.
    pub fn from_byte(byte: u8, blink_enabled: bool) -> Self {
        if blink_enabled {
            Self {
                foreground: byte & 0x0F,
                background: (byte >> 4) & 0x07,
                blink: (byte & 0x80) != 0,
            }
        } else {
            Self {
                foreground: byte & 0x0F,
                background: (byte >> 4) & 0x0F,
                blink: false,
            }
        }
    }

    /// Convert to VGA attribute byte (always uses blink-mode encoding)
    pub fn to_byte(&self) -> u8 {
        let mut byte = self.foreground & 0x0F;
        byte |= (self.background & 0x07) << 4;
        if self.blink {
            byte |= 0x80;
        }
        byte
    }
}

impl Default for TextAttribute {
    fn default() -> Self {
        Self {
            foreground: colors::LIGHT_GRAY,
            background: colors::BLACK,
            blink: false,
        }
    }
}

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
