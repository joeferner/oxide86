/// Video card type enumeration for emulation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoCardType {
    /// Monochrome Display Adapter (MDA) - text-only, 80x25 mono, CRTC at 0x3B4
    MDA,
    /// Hercules Graphics Card (HGC) - MDA-compatible plus 720x348 mono graphics
    HGC,
    /// Color Graphics Adapter (CGA) - supports text modes and CGA graphics (modes 0x00-0x07)
    CGA,
    /// Enhanced Graphics Adapter (EGA) - supports CGA modes plus 16-color graphics (modes 0x0D, 0x0E)
    #[default]
    EGA,
    /// Video Graphics Array (VGA) - supports EGA modes plus additional VGA modes
    VGA,
}

impl VideoCardType {
    /// Parse video card type from string (e.g., "cga", "ega", "vga")
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "mda" => Some(Self::MDA),
            "hgc" => Some(Self::HGC),
            "cga" => Some(Self::CGA),
            "ega" => Some(Self::EGA),
            "vga" => Some(Self::VGA),
            _ => None,
        }
    }

    /// Get the display name for this video card type
    pub fn name(&self) -> &'static str {
        match self {
            Self::MDA => "MDA",
            Self::HGC => "HGC",
            Self::CGA => "CGA",
            Self::EGA => "EGA",
            Self::VGA => "VGA",
        }
    }

    /// Check if this video card supports the given video mode number
    pub fn supports_mode(&self, mode: u8) -> bool {
        match self {
            Self::MDA | Self::HGC => matches!(mode, 0x00..=0x07),
            Self::CGA => matches!(mode, 0x00..=0x07),
            Self::EGA => matches!(mode, 0x00..=0x07 | 0x0D | 0x0E),
            Self::VGA => true,
        }
    }

    /// Get the INT 10h AH=1Ah display combination code for this card type
    /// Returns the active display code (BL value)
    pub fn display_combination_code(&self) -> u8 {
        match self {
            Self::MDA => 0x01, // MDA with monochrome display
            Self::HGC => 0x05, // HGC with monochrome display
            Self::CGA => 0x02, // CGA with color display
            Self::EGA => 0x04, // EGA with color display
            Self::VGA => 0x08, // VGA with color analog display
        }
    }

    /// Returns true if this is a monochrome adapter (MDA or HGC)
    pub fn is_monochrome(&self) -> bool {
        matches!(self, Self::MDA | Self::HGC)
    }
}

impl std::fmt::Display for VideoCardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
