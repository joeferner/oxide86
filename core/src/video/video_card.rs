use std::sync::Arc;

use crate::{
    Device,
    video::{CGA_MEMORY_END, CGA_MEMORY_SIZE, CGA_MEMORY_START, VideoBuffer},
};

pub struct VideoCard {
    buffer: Arc<VideoBuffer>,
    vram_size: usize,
}

impl VideoCard {
    pub fn new(buffer: Arc<VideoBuffer>) -> Self {
        Self {
            buffer,
            vram_size: CGA_MEMORY_SIZE, // TODO change based on video card type
        }
    }

    fn _read_u8(&self, addr: usize) -> u8 {
        // Read from raw VRAM (source of truth)
        if addr < self.vram_size {
            self.buffer.emu_get_back_buffer().vram[addr]
        } else {
            0
        }
    }

    fn _write_u8(&mut self, addr: usize, val: u8) {
        if addr < self.vram_size {
            log::info!("Write: [0x{addr:04X}] = 0x{val:02X}");
            self.buffer.emu_get_back_buffer_mut().vram[addr] = val;
        }
    }
}

impl Device for VideoCard {
    fn read_u8(&self, addr: usize) -> Option<u8> {
        // Route to Video for memory-mapped ranges
        if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&addr) {
            let offset = addr - CGA_MEMORY_START;
            Some(self._read_u8(offset))
        } else {
            None
        }
    }

    fn write_u8(&mut self, addr: usize, val: u8) -> bool {
        // Route to Video for memory-mapped ranges
        if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&addr) {
            let offset = addr - CGA_MEMORY_START;
            self._write_u8(offset, val);
            true
        } else {
            false
        }
    }
}
