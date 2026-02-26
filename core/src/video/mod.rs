// CGA video memory constants
pub const CGA_MEMORY_START: usize = 0xB8000;
pub const CGA_MEMORY_END: usize = 0xBFFFF;
pub const CGA_MEMORY_SIZE: usize = CGA_MEMORY_END - CGA_MEMORY_START + 1; // 32KB

pub struct VideoCard {
    /// Raw video RAM.
    vram: Vec<u8>,
}

impl VideoCard {
    pub fn new() -> Self {
        Self {
            vram: vec![0; CGA_MEMORY_SIZE],
        }
    }

    pub fn read_u8(&self, addr: usize) -> u8 {
        // Read from raw VRAM (source of truth)
        if addr < self.vram.len() {
            self.vram[addr]
        } else {
            0
        }
    }

    pub fn write_u8(&mut self, addr: usize, val: u8) {
        if addr < self.vram.len() {
            self.vram[addr] = val;
            log::info!("Write: [0x{addr:04X}] = 0x{val:02X}");
            // TODO draw
        }
    }
}
