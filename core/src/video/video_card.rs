use std::sync::Arc;

use crate::video::{CGA_MEMORY_SIZE, VideoBuffer};

pub struct VideoCard {
    buffer: Arc<VideoBuffer>,

    /// Raw video RAM.
    vram: Vec<u8>,
}

impl VideoCard {
    pub fn new(buffer: Arc<VideoBuffer>) -> Self {
        Self {
            buffer,
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
