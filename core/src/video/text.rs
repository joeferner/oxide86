use crate::video::colors;

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
