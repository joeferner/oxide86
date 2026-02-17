use crate::colors;

/// CGA color palette state
#[derive(Debug, Clone, Copy)]
pub struct CgaPalette {
    /// Background color (4 bits, 16 colors)
    pub background: u8,
    /// Palette select (0 or 1)
    pub palette_id: u8,
    /// Intensity/bright mode enabled
    pub intensity: bool,
}

impl CgaPalette {
    pub fn new() -> Self {
        Self {
            background: 0,
            palette_id: 0,
            intensity: false,
        }
    }

    /// Get the 4 colors for current palette
    /// Returns [background, color1, color2, color3]
    pub fn get_colors(&self) -> [u8; 4] {
        let bg = self.background;

        if self.palette_id == 0 {
            // Palette 0 (bit 5 = 0): Green, Red, Brown
            if self.intensity {
                [bg, colors::LIGHT_GREEN, colors::LIGHT_RED, colors::YELLOW]
            } else {
                // Use actual CGA hardware colors for accuracy
                [bg, colors::GREEN, colors::RED, colors::BROWN]
            }
        } else {
            // Palette 1 (bit 5 = 1): Cyan, Magenta, Light Gray/White
            if self.intensity {
                [bg, colors::LIGHT_CYAN, colors::LIGHT_MAGENTA, colors::WHITE]
            } else {
                // Use actual CGA hardware color (Light Gray)
                // On period monitors this appeared bright/white-ish
                [bg, colors::CYAN, colors::MAGENTA, colors::LIGHT_GRAY]
            }
        }
    }

    /// Parse from CGA Color Select Register (port 0x3D9)
    pub fn from_register(value: u8) -> Self {
        Self {
            background: value & 0x0F,
            palette_id: (value >> 5) & 0x01,
            intensity: (value & 0x10) != 0,
        }
    }

    /// Convert to Color Select Register value
    pub fn to_register(&self) -> u8 {
        let mut value = self.background & 0x0F;
        if self.intensity {
            value |= 0x10;
        }
        value |= (self.palette_id & 0x01) << 5;
        value
    }
}

impl Default for CgaPalette {
    fn default() -> Self {
        Self::new()
    }
}
