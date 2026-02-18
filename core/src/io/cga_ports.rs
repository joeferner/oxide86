/// CGA Mode Control Register (port 0x3D8)
#[derive(Debug, Clone, Copy)]
pub struct CgaModeControl {
    value: u8,
}

impl CgaModeControl {
    pub fn new() -> Self {
        Self { value: 0x00 }
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
