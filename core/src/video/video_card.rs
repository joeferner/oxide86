use std::{
    any::Any,
    cell::Cell,
    sync::{Arc, RwLock},
};

use crate::{
    Device,
    video::{
        CGA_MEMORY_END, CGA_MEMORY_SIZE, CGA_MEMORY_START, EGA_MEMORY_END, EGA_MEMORY_START,
        EGA_PLANE_SIZE, TEXT_MODE_COLS, TEXT_MODE_ROWS, VIDEO_MEMORY_SIZE,
        VIDEO_MODE_0DH_EGA_320_X_200_16, VIDEO_MODE_02H_COLOR_TEXT_80_X_25,
        VIDEO_MODE_03H_COLOR_TEXT_80_X_25, VIDEO_MODE_04H_CGA_320_X_200_4,
        VIDEO_MODE_06H_CGA_640_X_200_2, VideoBuffer, VideoCardType,
        font::{CHAR_HEIGHT_8, Cp437Font},
    },
};

// CGA/EGA/VGA CRTC ports
pub const VIDEO_CARD_CONTROL_ADDR: u16 = 0x03D4;
pub const VIDEO_CARD_DATA_ADDR: u16 = 0x03D5;
pub const CGA_COLOR_SELECT_ADDR: u16 = 0x03D9;

// MDA CRTC ports (same 6845 chip but different address; none of our card types are MDA)
const MDA_CRTC_CONTROL_ADDR: u16 = 0x03B4;
const MDA_CRTC_DATA_ADDR: u16 = 0x03B5;

pub const VIDEO_CARD_REG_CURSOR_START_LINE: u8 = 0x0a;
pub const VIDEO_CARD_REG_CURSOR_END_LINE: u8 = 0x0b;
pub const VIDEO_CARD_START_ADDRESS_HIGH_REGISTER: u8 = 0x0c;
pub const VIDEO_CARD_START_ADDRESS_LOW_REGISTER: u8 = 0x0d;
pub const VIDEO_CARD_REG_CURSOR_LOC_HIGH: u8 = 0x0e;
pub const VIDEO_CARD_REG_CURSOR_LOC_LOW: u8 = 0x0f;

// EGA/VGA Attribute Controller ports
pub const AC_ADDR_DATA_PORT: u16 = 0x3C0;
pub const AC_DATA_READ_PORT: u16 = 0x3C1;
pub const AC_REG_MODE_CONTROL: u8 = 0x10;
pub const AC_REG_COLOR_SELECT: u8 = 0x14;
// VGA DAC ports
pub const DAC_READ_INDEX_PORT: u16 = 0x3C7;
pub const DAC_WRITE_INDEX_PORT: u16 = 0x3C8;
pub const DAC_DATA_PORT: u16 = 0x3C9;
// Input Status Register 1 (resets AC flip-flop on read)
pub const INPUT_STATUS_1_PORT: u16 = 0x3DA;

pub(crate) struct ModeInfo {
    pub rows: u8,
    pub cols: u8,
}

/// CGA vertical refresh rate.
const CGA_VSYNC_HZ: u64 = 60;
/// Vsync active for roughly 1/12 of the frame (~8%).
const CGA_VSYNC_DUTY_DIVISOR: u64 = 12;

pub struct VideoCard {
    card_type: VideoCardType,
    buffer: Arc<RwLock<VideoBuffer>>,
    vram_size: usize,
    cpu_clock_speed: u32,
    io_register: u8,
    color_select: u8,
    // EGA/VGA Attribute Controller registers (16 palette + 1 border color)
    ac_registers: [u8; 17],
    ac_address: u8,
    ac_flip_flop: Cell<bool>, // false = address mode, true = data mode
    // VGA DAC registers (256 entries, each RGB 0-63)
    dac_registers: Vec<[u8; 3]>,
    dac_write_pos: usize, // index * 3 + color_component
    dac_read_pos: Cell<usize>,
    // EGA/VGA Sequencer registers
    sequencer_address: u8,
    /// Map Mask register (sequencer index 0x02): bit N = enable write to plane N.
    /// Default 0x0F = all 4 planes enabled.
    sequencer_map_mask: u8,
    // EGA/VGA Graphics Controller registers
    gc_address: u8,
    /// Read Map Select (GC register 0x04): which plane (0-3) the CPU reads from.
    gc_read_map_select: u8,
}

impl VideoCard {
    pub fn new(
        card_type: VideoCardType,
        buffer: Arc<RwLock<VideoBuffer>>,
        cpu_clock_speed: u32,
    ) -> Self {
        Self {
            card_type,
            buffer,
            vram_size: match card_type {
                VideoCardType::EGA | VideoCardType::VGA => VIDEO_MEMORY_SIZE,
                VideoCardType::CGA => CGA_MEMORY_SIZE,
            },
            cpu_clock_speed,
            io_register: 0,
            color_select: 0,
            ac_registers: [0u8; 17],
            ac_address: 0,
            ac_flip_flop: Cell::new(false),
            dac_registers: vec![[0u8; 3]; 256],
            dac_write_pos: 0,
            dac_read_pos: Cell::new(0),
            sequencer_address: 0,
            sequencer_map_mask: 0x0F,
            gc_address: 0,
            gc_read_map_select: 0,
        }
    }

    pub(crate) fn card_type(&self) -> VideoCardType {
        self.card_type
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
            buffer.write_vram(addr, val);
        }
    }

    pub(crate) fn set_mode(&mut self, mode: u8) -> Option<ModeInfo> {
        log::info!("set mode: 0x{mode:02X}");
        self.buffer.write().unwrap().set_mode(mode);
        if mode == VIDEO_MODE_02H_COLOR_TEXT_80_X_25 || mode == VIDEO_MODE_03H_COLOR_TEXT_80_X_25 {
            Some(ModeInfo {
                rows: TEXT_MODE_ROWS as u8,
                cols: TEXT_MODE_COLS as u8,
            })
        } else if mode == VIDEO_MODE_04H_CGA_320_X_200_4 {
            Some(ModeInfo { rows: 25, cols: 40 })
        } else if mode == VIDEO_MODE_06H_CGA_640_X_200_2 {
            Some(ModeInfo { rows: 25, cols: 80 })
        } else if mode == VIDEO_MODE_0DH_EGA_320_X_200_16 {
            Some(ModeInfo { rows: 25, cols: 40 })
        } else {
            None
        }
    }

    /// Draw a character transparently into EGA planar VRAM.
    ///
    /// Foreground pixels (glyph bit = 1) are set to `fg_color` in all planes.
    /// Background pixels (glyph bit = 0) are left unchanged (transparent).
    ///
    /// `char_row` and `char_col` are character-cell coordinates (40 cols × 25 rows).
    pub(crate) fn ega_draw_char_transparent(
        &self,
        ch: u8,
        char_row: u8,
        char_col: u8,
        fg_color: u8,
    ) {
        let font = Cp437Font::new();
        let glyph = font.get_glyph_8(ch);
        let mut buffer = self.buffer.write().unwrap();
        for (r, &glyph_byte) in glyph.iter().enumerate().take(CHAR_HEIGHT_8) {
            let pixel_y = char_row as usize * CHAR_HEIGHT_8 + r;
            let byte_offset = pixel_y * 40 + char_col as usize;
            for plane in 0..4u8 {
                let plane_vram = plane as usize * EGA_PLANE_SIZE + byte_offset;
                if plane_vram >= buffer.vram_len() {
                    continue;
                }
                let plane_bit = (fg_color >> plane) & 1;
                if plane_bit != 0 {
                    let existing = buffer.read_vram(plane_vram);
                    buffer.write_vram(plane_vram, existing | glyph_byte);
                } else {
                    let existing = buffer.read_vram(plane_vram);
                    buffer.write_vram(plane_vram, existing & !glyph_byte);
                }
            }
        }
    }
}

impl Device for VideoCard {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        let mut buffer = self.buffer.write().unwrap();
        buffer.reset();
        self.vram_size = match self.card_type {
            VideoCardType::EGA | VideoCardType::VGA => VIDEO_MEMORY_SIZE,
            VideoCardType::CGA => CGA_MEMORY_SIZE,
        };
        self.io_register = 0;
        self.color_select = 0;
        self.ac_registers = [0u8; 17];
        self.ac_address = 0;
        self.ac_flip_flop.set(false);
        self.dac_registers = vec![[0u8; 3]; 256];
        self.dac_write_pos = 0;
        self.dac_read_pos.set(0);
        self.sequencer_address = 0;
        self.sequencer_map_mask = 0x0F;
        self.gc_address = 0;
        self.gc_read_map_select = 0;
    }

    fn memory_read_u8(&self, addr: usize, _cycle_count: u32) -> Option<u8> {
        if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&addr) {
            let offset = addr - CGA_MEMORY_START;
            Some(self.internal_read_u8(offset))
        } else if matches!(self.card_type, VideoCardType::EGA | VideoCardType::VGA)
            && (EGA_MEMORY_START..=EGA_MEMORY_END).contains(&addr)
        {
            let offset = addr - EGA_MEMORY_START;
            if offset < EGA_PLANE_SIZE {
                let plane_offset = self.gc_read_map_select as usize * EGA_PLANE_SIZE + offset;
                Some(self.internal_read_u8(plane_offset))
            } else {
                Some(0xFF)
            }
        } else {
            None
        }
    }

    fn memory_write_u8(&mut self, addr: usize, val: u8, _cycle_count: u32) -> bool {
        if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&addr) {
            let offset = addr - CGA_MEMORY_START;
            self.internal_write_u8(offset, val);
            true
        } else if matches!(self.card_type, VideoCardType::EGA | VideoCardType::VGA)
            && (EGA_MEMORY_START..=EGA_MEMORY_END).contains(&addr)
        {
            let offset = addr - EGA_MEMORY_START;
            if offset < EGA_PLANE_SIZE {
                for plane in 0..4u8 {
                    if (self.sequencer_map_mask >> plane) & 1 != 0 {
                        let plane_offset = plane as usize * EGA_PLANE_SIZE + offset;
                        self.internal_write_u8(plane_offset, val);
                    }
                }
            }
            true
        } else {
            false
        }
    }

    fn io_read_u8(&self, port: u16, cycle_count: u32) -> Option<u8> {
        let is_ega_vga = matches!(self.card_type, VideoCardType::EGA | VideoCardType::VGA);
        match self.card_type {
            VideoCardType::CGA | VideoCardType::EGA | VideoCardType::VGA => match port {
                // MDA ports: return 0xFF — no MDA card present
                MDA_CRTC_CONTROL_ADDR | MDA_CRTC_DATA_ADDR => Some(0xFF),
                VIDEO_CARD_DATA_ADDR => {
                    let buffer = self.buffer.read().unwrap();
                    let val = match self.io_register {
                        VIDEO_CARD_REG_CURSOR_LOC_HIGH => (buffer.cursor_loc() >> 8) as u8,
                        VIDEO_CARD_REG_CURSOR_LOC_LOW => (buffer.cursor_loc() & 0xFF) as u8,
                        VIDEO_CARD_START_ADDRESS_HIGH_REGISTER => {
                            (buffer.start_address() >> 8) as u8
                        }
                        VIDEO_CARD_START_ADDRESS_LOW_REGISTER => {
                            (buffer.start_address() & 0xFF) as u8
                        }
                        _ => 0,
                    };
                    Some(val)
                }
                CGA_COLOR_SELECT_ADDR => Some(self.color_select),
                // Sequencer address/data read (EGA/VGA only)
                0x3C4 if is_ega_vga => Some(self.sequencer_address),
                0x3C5 if is_ega_vga => Some(match self.sequencer_address {
                    0x02 => self.sequencer_map_mask,
                    _ => 0,
                }),
                // AC data read (EGA/VGA only)
                0x3C1 if is_ega_vga => Some(self.ac_registers[(self.ac_address & 0x0F) as usize]),
                // Graphics Controller address/data read (EGA/VGA only)
                0x3CE if is_ega_vga => Some(self.gc_address),
                0x3CF if is_ega_vga => Some(match self.gc_address {
                    0x04 => self.gc_read_map_select,
                    _ => 0,
                }),
                // DAC data read (VGA only)
                0x3C9 if is_ega_vga => {
                    let pos = self.dac_read_pos.get();
                    let reg = pos / 3;
                    let component = pos % 3;
                    self.dac_read_pos.set((pos + 1) % (256 * 3));
                    Some(self.dac_registers[reg][component])
                }
                // Input Status Register 1: resets AC flip-flop to address mode.
                // Bit 3: vertical retrace (vsync) active. Simulated at ~60Hz using cycle count.
                0x3DA => {
                    self.ac_flip_flop.set(false);
                    let cycles_per_frame = self.cpu_clock_speed as u64 / CGA_VSYNC_HZ;
                    let vsync_cycles = cycles_per_frame / CGA_VSYNC_DUTY_DIVISOR;
                    let phase = cycle_count as u64 % cycles_per_frame;
                    let in_vsync = phase < vsync_cycles;
                    Some(if in_vsync { 0x08 } else { 0x00 })
                }
                _ => None,
            },
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        let is_ega_vga = matches!(self.card_type, VideoCardType::EGA | VideoCardType::VGA);
        match self.card_type {
            VideoCardType::CGA | VideoCardType::EGA | VideoCardType::VGA => match port {
                // MDA ports: silently ignore — no MDA card present
                MDA_CRTC_CONTROL_ADDR | MDA_CRTC_DATA_ADDR => true,
                CGA_COLOR_SELECT_ADDR => {
                    self.color_select = val;
                    let mut buffer = self.buffer.write().unwrap();
                    buffer.set_cga_color_select(
                        (val & 0x0F) as usize,
                        (val & 0x10) != 0,
                        (val & 0x20) != 0,
                    );
                    true
                }
                VIDEO_CARD_CONTROL_ADDR => {
                    self.io_register = val;
                    true
                }
                VIDEO_CARD_DATA_ADDR => {
                    let mut buffer = self.buffer.write().unwrap();
                    match self.io_register {
                        VIDEO_CARD_REG_CURSOR_START_LINE => {
                            buffer.set_cursor_visible((val & 0x20) == 0);
                            buffer.set_cursor_start_line(val & 0x1F);
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
                            let new_cursor_loc =
                                (buffer.cursor_loc() & 0x00ff) | ((val as u16) << 8);
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
                }
                // Sequencer address register (EGA/VGA only)
                0x3C4 if is_ega_vga => {
                    self.sequencer_address = val;
                    true
                }
                // Sequencer data register (EGA/VGA only)
                0x3C5 if is_ega_vga => {
                    match self.sequencer_address {
                        0x02 => self.sequencer_map_mask = val,
                        _ => log::warn!(
                            "Unhandled sequencer register 0x{:02X} = 0x{:02X}",
                            self.sequencer_address,
                            val
                        ),
                    }
                    true
                }
                // AC address/data write (EGA/VGA only) — flip-flop toggles address vs data
                0x3C0 if is_ega_vga => {
                    if !self.ac_flip_flop.get() {
                        self.ac_address = val & 0x1F;
                        self.ac_flip_flop.set(true);
                    } else {
                        self.ac_registers[(self.ac_address & 0x0F) as usize] = val;
                        self.ac_flip_flop.set(false);
                    }
                    true
                }
                // Graphics Controller address/data write (EGA/VGA only)
                0x3CE if is_ega_vga => {
                    self.gc_address = val;
                    true
                }
                0x3CF if is_ega_vga => {
                    match self.gc_address {
                        0x04 => self.gc_read_map_select = val & 0x03,
                        _ => log::warn!(
                            "Unhandled GC register 0x{:02X} = 0x{:02X}",
                            self.gc_address,
                            val
                        ),
                    }
                    true
                }
                // DAC read index (EGA/VGA only)
                0x3C7 if is_ega_vga => {
                    self.dac_read_pos.set((val as usize) * 3);
                    true
                }
                // DAC write index (EGA/VGA only)
                0x3C8 if is_ega_vga => {
                    self.dac_write_pos = (val as usize) * 3;
                    true
                }
                // DAC data write (EGA/VGA only) — cycles R, G, B
                0x3C9 if is_ega_vga => {
                    let reg = self.dac_write_pos / 3;
                    let component = self.dac_write_pos % 3;
                    self.dac_registers[reg][component] = val & 0x3F;
                    self.dac_write_pos = (self.dac_write_pos + 1) % (256 * 3);
                    true
                }
                _ => false,
            },
        }
    }
}
