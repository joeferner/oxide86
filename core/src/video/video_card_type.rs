/// Video card type enumeration for emulation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoCardType {
    /// Color Graphics Adapter (CGA) - supports text modes and CGA graphics (modes 0x00-0x07)
    CGA,
    /// Enhanced Graphics Adapter (EGA) - supports CGA modes plus 16-color graphics (mode 0x0D)
    #[default]
    EGA,
    /// Video Graphics Array (VGA) - supports EGA modes plus additional VGA modes
    VGA,
}

impl VideoCardType {
    /// Parse video card type from string (e.g., "cga", "ega", "vga")
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cga" => Some(Self::CGA),
            "ega" => Some(Self::EGA),
            "vga" => Some(Self::VGA),
            _ => None,
        }
    }

    /// Get the display name for this video card type
    pub fn name(&self) -> &'static str {
        match self {
            Self::CGA => "CGA",
            Self::EGA => "EGA",
            Self::VGA => "VGA",
        }
    }

    /// Check if this video card supports the given video mode number
    pub fn supports_mode(&self, mode: u8) -> bool {
        match self {
            Self::CGA => matches!(mode, 0x00..=0x07),
            Self::EGA => matches!(mode, 0x00..=0x07 | 0x0D),
            Self::VGA => true,
        }
    }

    /// Get the INT 10h AH=1Ah display combination code for this card type
    /// Returns the active display code (BL value)
    pub fn display_combination_code(&self) -> u8 {
        match self {
            Self::CGA => 0x02, // CGA with color display
            Self::EGA => 0x04, // EGA with color display
            Self::VGA => 0x08, // VGA with color analog display
        }
    }
}

impl std::fmt::Display for VideoCardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
