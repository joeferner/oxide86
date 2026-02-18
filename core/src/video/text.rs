use std::ops::{Index, IndexMut};

use crate::{
    colors,
    video::{TEXT_MODE_COLS, TEXT_MODE_ROWS},
};

/// VGA text mode character attribute
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextAttribute {
    pub foreground: u8, // 4 bits
    pub background: u8, // 3 bits
    pub blink: bool,    // 1 bit
}

impl TextAttribute {
    /// Create from VGA attribute byte
    pub fn from_byte(byte: u8) -> Self {
        Self {
            foreground: byte & 0x0F,
            background: (byte >> 4) & 0x07,
            blink: (byte & 0x80) != 0,
        }
    }

    /// Convert to VGA attribute byte
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
