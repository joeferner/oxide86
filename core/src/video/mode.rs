use crate::video::font::{CHAR_HEIGHT, CHAR_HEIGHT_8, CHAR_WIDTH};
use crate::video::{TEXT_MODE_COLS, TEXT_MODE_ROWS, VGA_MODE_13_HEIGHT, VGA_MODE_13_WIDTH};
use core::fmt;

pub struct TextDimensions {
    pub rows: usize,
    pub cols: usize,
}

#[derive(Clone)]
pub enum Mode {
    M00ColorText40,
    M01Text40,
    M02ColorText,
    M03Text,
    M04Cga320x200x4,
    M06Cga640x200x2,
    M0DEga320x200x16,
    M10Ega640x350x16,
    M13Vga320x200x256,
    Unknown(u8),
}

pub const TEXT_MODE_COLS_40: usize = 40;

impl Mode {
    pub fn as_u8(&self) -> u8 {
        match self {
            Mode::M00ColorText40 => 0x00,
            Mode::M01Text40 => 0x01,
            Mode::M02ColorText => 0x02,
            Mode::M03Text => 0x03,
            Mode::M04Cga320x200x4 => 0x04,
            Mode::M06Cga640x200x2 => 0x06,
            Mode::M0DEga320x200x16 => 0x0d,
            Mode::M10Ega640x350x16 => 0x10,
            Mode::M13Vga320x200x256 => 0x13,
            Mode::Unknown(v) => *v,
        }
    }

    pub fn resolution(&self) -> (u32, u32) {
        match self {
            Mode::M00ColorText40 | Mode::M01Text40 => (
                (CHAR_WIDTH * TEXT_MODE_COLS_40) as u32,
                (CHAR_HEIGHT_8 * TEXT_MODE_ROWS) as u32,
            ),
            Mode::M04Cga320x200x4 => (320, 200),
            Mode::M06Cga640x200x2 => (640, 400),
            Mode::M0DEga320x200x16 => (320, 200),
            Mode::M10Ega640x350x16 => (640, 350),
            Mode::M13Vga320x200x256 => (VGA_MODE_13_WIDTH as u32, VGA_MODE_13_HEIGHT as u32),
            _ => (
                (CHAR_WIDTH * TEXT_MODE_COLS) as u32,
                (CHAR_HEIGHT * TEXT_MODE_ROWS) as u32,
            ),
        }
    }

    pub fn get_text_dimensions(&self) -> Option<TextDimensions> {
        match self {
            Mode::M00ColorText40 => Some(TextDimensions { rows: 25, cols: 40 }),
            Mode::M01Text40 => Some(TextDimensions { rows: 25, cols: 40 }),
            Mode::M02ColorText => Some(TextDimensions { rows: 25, cols: 80 }),
            Mode::M03Text => Some(TextDimensions { rows: 25, cols: 80 }),
            Mode::M04Cga320x200x4 => Some(TextDimensions { rows: 25, cols: 40 }),
            Mode::M06Cga640x200x2 => Some(TextDimensions { rows: 25, cols: 80 }),
            Mode::M0DEga320x200x16 => Some(TextDimensions { rows: 25, cols: 40 }),
            Mode::M10Ega640x350x16 => Some(TextDimensions { rows: 25, cols: 80 }),
            Mode::M13Vga320x200x256 => Some(TextDimensions { rows: 25, cols: 40 }),
            Mode::Unknown(_) => None,
        }
    }
}

impl From<u8> for Mode {
    fn from(val: u8) -> Self {
        match val {
            0x00 => Mode::M00ColorText40,
            0x01 => Mode::M01Text40,
            0x02 => Mode::M02ColorText,
            0x03 => Mode::M03Text,
            0x04 => Mode::M04Cga320x200x4,
            0x06 => Mode::M06Cga640x200x2,
            0x0d => Mode::M0DEga320x200x16,
            0x10 => Mode::M10Ega640x350x16,
            0x13 => Mode::M13Vga320x200x256,
            v => Mode::Unknown(v),
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:02X}", self.as_u8())
    }
}
