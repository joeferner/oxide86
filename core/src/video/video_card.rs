use std::{
    any::Any,
    sync::{Arc, RwLock},
};

use crate::{
    Device,
    video::{CGA_MEMORY_END, CGA_MEMORY_SIZE, CGA_MEMORY_START, VideoBuffer},
};

pub const VIDEO_CARD_CONTROL_ADDR: u16 = 0x03D4;
pub const VIDEO_CARD_DATA_ADDR: u16 = 0x03D5;

pub const VIDEO_CARD_REG_CURSOR_LOC_HIGH: u8 = 0x0E;
pub const VIDEO_CARD_REG_CURSOR_LOC_LOW: u8 = 0x0F;

pub struct VideoCard {
    buffer: Arc<RwLock<VideoBuffer>>,
    vram_size: usize,
    io_register: u8,
}

impl VideoCard {
    pub fn new(buffer: Arc<RwLock<VideoBuffer>>) -> Self {
        Self {
            buffer,
            vram_size: CGA_MEMORY_SIZE, // TODO change based on video card type
            io_register: 0,
        }
    }

    fn internal_read_u8(&self, addr: usize) -> u8 {
        // Read from raw VRAM (source of truth)
        if addr < self.vram_size {
            let buffer = self.buffer.read().unwrap();
            buffer.read_vram(addr)
        } else {
            0
        }
    }

    fn internal_write_u8(&mut self, addr: usize, val: u8) {
        if addr < self.vram_size {
            let mut buffer = self.buffer.write().unwrap();
            log::debug!("Write: [0x{addr:04X}] = 0x{val:02X}");
            buffer.write_vram(addr, val);
        }
    }
}

impl Device for VideoCard {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn memory_read_u8(&self, addr: usize) -> Option<u8> {
        // Route to Video for memory-mapped ranges
        if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&addr) {
            let offset = addr - CGA_MEMORY_START;
            Some(self.internal_read_u8(offset))
        } else {
            None
        }
    }

    fn memory_write_u8(&mut self, addr: usize, val: u8) -> bool {
        // Route to Video for memory-mapped ranges
        if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&addr) {
            let offset = addr - CGA_MEMORY_START;
            self.internal_write_u8(offset, val);
            true
        } else {
            false
        }
    }

    fn io_read_u8(&self, _port: u16) -> Option<u8> {
        None
    }

    fn io_write_u8(&mut self, port: u16, val: u8) -> bool {
        if port == VIDEO_CARD_CONTROL_ADDR {
            self.io_register = val;
            true
        } else if port == VIDEO_CARD_DATA_ADDR {
            let mut buffer = self.buffer.write().unwrap();
            match self.io_register {
                VIDEO_CARD_REG_CURSOR_LOC_HIGH => {
                    let new_cursor_loc = (buffer.cursor_loc() & 0x00ff) | ((val as u16) << 8);
                    buffer.set_cursor_loc(new_cursor_loc);
                }
                VIDEO_CARD_REG_CURSOR_LOC_LOW => {
                    let new_cursor_loc = (buffer.cursor_loc() & 0xff00) | val as u16;
                    buffer.set_cursor_loc(new_cursor_loc);
                }
                _ => log::warn!("invalid IO Register: 0x{:04X}", self.io_register),
            }
            true
        } else {
            false
        }
    }
}
