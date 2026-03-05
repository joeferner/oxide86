use crate::video::colors;

/// Convert CP437 byte to Unicode character
/// CP437 is the original IBM PC character set
pub fn cp437_to_unicode(byte: u8) -> char {
    // CP437 high characters (0x80-0xFF) to Unicode
    const CP437_HIGH: [char; 128] = [
        'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ',
        'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú',
        'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡',
        '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟',
        '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘',
        '┌', '█', '▄', '▌', '▐', '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ',
        '∞', 'φ', 'ε', '∩', '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²',
        '■', ' ',
    ];

    match byte {
        0x00 => ' ',                 // NUL - display as space
        0x20..=0x7E => byte as char, // Standard ASCII printable
        0x7F => '⌂',                 // DEL - house symbol in CP437
        0x80..=0xFF => CP437_HIGH[(byte - 0x80) as usize],
        _ => byte as char, // Low control chars - pass through
    }
}

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

    // Convert to VGA attribute byte (always uses blink-mode encoding)
    // TODO
    // pub(crate) fn to_byte(&self) -> u8 {
    //     let mut byte = self.foreground & 0x0F;
    //     byte |= (self.background & 0x07) << 4;
    //     if self.blink {
    //         byte |= 0x80;
    //     }
    //     byte
    // }
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
