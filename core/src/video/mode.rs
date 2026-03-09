use core::fmt;

pub struct TextDimensions {
    pub rows: usize,
    pub cols: usize,
}

#[derive(Clone)]
pub enum Mode {
    M02ColorText,
    M03Text,
    M04Cga320x200x4,
    M06Cga640x200x2,
    M0DEga320x200x16,
    M10Ega640x350x16,
    Unknown(u8),
}

impl Mode {
    pub fn as_u8(&self) -> u8 {
        match self {
            Mode::M02ColorText => 0x02,
            Mode::M03Text => 0x03,
            Mode::M04Cga320x200x4 => 0x04,
            Mode::M06Cga640x200x2 => 0x06,
            Mode::M0DEga320x200x16 => 0x0d,
            Mode::M10Ega640x350x16 => 0x10,
            Mode::Unknown(v) => *v,
        }
    }

    pub fn resolution(&self) -> (u32, u32) {
        use crate::video::font::{CHAR_HEIGHT, CHAR_WIDTH};
        use crate::video::{TEXT_MODE_COLS, TEXT_MODE_ROWS};
        match self {
            Mode::M04Cga320x200x4 => (320, 200),
            Mode::M06Cga640x200x2 => (640, 400),
            Mode::M0DEga320x200x16 => (320, 200),
            Mode::M10Ega640x350x16 => (640, 350),
            _ => (
                (CHAR_WIDTH * TEXT_MODE_COLS) as u32,
                (CHAR_HEIGHT * TEXT_MODE_ROWS) as u32,
            ),
        }
    }

    pub fn get_text_dimensions(&self) -> Option<TextDimensions> {
        match self {
            Mode::M02ColorText => Some(TextDimensions { rows: 25, cols: 80 }),
            Mode::M03Text => Some(TextDimensions { rows: 25, cols: 80 }),
            Mode::M04Cga320x200x4 => Some(TextDimensions { rows: 25, cols: 40 }),
            Mode::M06Cga640x200x2 => Some(TextDimensions { rows: 25, cols: 80 }),
            Mode::M0DEga320x200x16 => Some(TextDimensions { rows: 25, cols: 40 }),
            Mode::M10Ega640x350x16 => Some(TextDimensions { rows: 25, cols: 80 }),
            Mode::Unknown(_) => None,
        }
    }
}

impl From<u8> for Mode {
    fn from(val: u8) -> Self {
        match val {
            0x02 => Mode::M02ColorText,
            0x03 => Mode::M03Text,
            0x04 => Mode::M04Cga320x200x4,
            0x06 => Mode::M06Cga640x200x2,
            0x0d => Mode::M0DEga320x200x16,
            0x10 => Mode::M10Ega640x350x16,
            v => Mode::Unknown(v),
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:02X}", self.as_u8())
    }
}
