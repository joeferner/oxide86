use std::{
    any::Any,
    sync::{Arc, RwLock},
};

use crate::{
    Device, byte_to_printable_char,
    video::{
        CGA_MEMORY_END, CGA_MEMORY_SIZE, CGA_MEMORY_START, TEXT_MODE_COLS, TEXT_MODE_ROWS,
        VIDEO_MODE_02H_COLOR_TEXT_80_X_25, VIDEO_MODE_03H_COLOR_TEXT_80_X_25, VideoBuffer,
    },
};

pub const VIDEO_CARD_CONTROL_ADDR: u16 = 0x03D4;
pub const VIDEO_CARD_DATA_ADDR: u16 = 0x03D5;

pub const VIDEO_CARD_REG_CURSOR_START_LINE: u8 = 0x0a;
pub const VIDEO_CARD_REG_CURSOR_END_LINE: u8 = 0x0b;
pub const VIDEO_CARD_START_ADDRESS_HIGH_REGISTER: u8 = 0x0c;
pub const VIDEO_CARD_START_ADDRESS_LOW_REGISTER: u8 = 0x0d;
pub const VIDEO_CARD_REG_CURSOR_LOC_HIGH: u8 = 0x0e;
pub const VIDEO_CARD_REG_CURSOR_LOC_LOW: u8 = 0x0f;

pub struct ModeInfo {
    pub rows: u8,
    pub cols: u8,
}

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
            log::debug!(
                "Write: [0x{addr:04X}] = 0x{val:02X} '{}'",
                byte_to_printable_char(val)
            );
            buffer.write_vram(addr, val);
        }
    }

    pub fn set_mode(&mut self, mode: u8, _clear_screen: bool) -> Option<ModeInfo> {
        if mode == VIDEO_MODE_02H_COLOR_TEXT_80_X_25 || mode == VIDEO_MODE_03H_COLOR_TEXT_80_X_25 {
            return Some(ModeInfo {
                rows: TEXT_MODE_ROWS as u8,
                cols: TEXT_MODE_COLS as u8,
            });
        }

        // TODO

        // // 3. Look up Mode Parameters in BIOS Video Parameter Table
        // params = VideoParameterTable[actual_mode]

        // // 4. Update the CRT Controller (CRTC) Registers
        // // These define screen resolution, timing, and refresh rates
        // FOR EACH register IN CRTC_Registers:
        //     WriteToPort(0x3D4, register.index)
        //     WriteToPort(0x3D5, params.value)
        // END FOR

        // // 5. Initialize the Sequencer and Graphics Controller
        // InitializeSequencer(params)
        // InitializeGraphicsController(params)

        // // 6. Set up the Attribute Controller (Palette and Colors)
        // InitializePalette(params.default_colors)

        // // 7. Clear Video Buffer (VRAM)
        // IF clear_screen_flag == TRUE THEN
        //     FillMemory(start_address: 0xA0000, size: 64KB, value: 0)
        // END IF

        // Check if the requested mode is supported by the video card type
        // if !bus.video().supports_mode(mode) {
        //     log::warn!(
        //         "INT 0x10 AH=0x00: Video mode 0x{:02X} not supported by {} card - ignoring",
        //         mode,
        //         bus.video().card_type()
        //     );
        //     return;
        // }

        // // INT 10h mode set = RGB rendering; composite only via port 0x3D8
        // bus.video_mut().set_composite_mode(false);
        // bus.video_mut().set_mode(mode, false); // INT 10h clears video memory (real BIOS behavior)
        // // Reset cursor to top-left (only relevant for text modes)
        // bus.video_mut().set_cursor(0, 0);

        // let cols = bus.video().get_cols();
        // let rows = bus.video().get_rows();

        None
    }
}

impl Device for VideoCard {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        let mut buffer = self.buffer.write().unwrap();
        buffer.reset();
        self.vram_size = CGA_MEMORY_SIZE; // TODO change based on video card type
        self.io_register = 0;
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
                VIDEO_CARD_REG_CURSOR_START_LINE => {
                    buffer.set_cursor_start_line(val);
                }
                VIDEO_CARD_REG_CURSOR_END_LINE => {
                    buffer.set_cursor_end_line(val);
                }
                VIDEO_CARD_START_ADDRESS_HIGH_REGISTER => {
                    let new_start = (buffer.start_address() & 0x00ff) | ((val as u16) << 8);
                    buffer.set_start_address(new_start);
                }
                VIDEO_CARD_START_ADDRESS_LOW_REGISTER => {
                    let new_start = (buffer.start_address() & 0xff00) | val as u16;
                    buffer.set_start_address(new_start);
                }
                VIDEO_CARD_REG_CURSOR_LOC_HIGH => {
                    let new_cursor_loc = (buffer.cursor_loc() & 0x00ff) | ((val as u16) << 8);
                    buffer.set_cursor_loc(new_cursor_loc);
                }
                VIDEO_CARD_REG_CURSOR_LOC_LOW => {
                    let new_cursor_loc = (buffer.cursor_loc() & 0xff00) | val as u16;
                    buffer.set_cursor_loc(new_cursor_loc);
                }
                _ => log::warn!(
                    "invalid IO Register: 0x{:04X} (val: 0x{:02X})",
                    self.io_register,
                    val
                ),
            }
            true
        } else {
            false
        }
    }
}
