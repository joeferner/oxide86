/// CGA Mode Control Register (port 0x3D8)
#[derive(Debug, Clone, Copy)]
pub struct CgaModeControl {
    value: u8,
}

impl CgaModeControl {
    pub fn new() -> Self {
        Self { value: 0x00 }
    }

    /// Bit 0: 80x25 text mode (0 = 40x25, 1 = 80x25)
    #[allow(dead_code)]
    pub fn is_80_column(&self) -> bool {
        (self.value & 0x01) != 0
    }

    /// Bit 1: Graphics mode enable (0 = text, 1 = graphics)
    #[allow(dead_code)]
    pub fn is_graphics(&self) -> bool {
        (self.value & 0x02) != 0
    }

    /// Bit 2: Monochrome mode (0 = color, 1 = monochrome)
    #[allow(dead_code)]
    pub fn is_monochrome(&self) -> bool {
        (self.value & 0x04) != 0
    }

    /// Bit 3: Video enable (0 = disabled, 1 = enabled)
    #[allow(dead_code)]
    pub fn is_video_enabled(&self) -> bool {
        (self.value & 0x08) != 0
    }

    /// Bit 4: High-resolution graphics (0 = 320x200, 1 = 640x200)
    #[allow(dead_code)]
    pub fn is_high_res(&self) -> bool {
        (self.value & 0x10) != 0
    }

    /// Bit 5: Blink enable (0 = intensity, 1 = blink)
    #[allow(dead_code)]
    pub fn is_blink_enabled(&self) -> bool {
        (self.value & 0x20) != 0
    }

    pub fn write(&mut self, value: u8) {
        self.value = value;
    }

    pub fn read(&self) -> u8 {
        self.value
    }
}

impl Default for CgaModeControl {
    fn default() -> Self {
        Self::new()
    }
}
