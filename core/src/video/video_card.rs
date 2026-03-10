use std::{
    any::Any,
    cell::Cell,
    sync::{Arc, RwLock},
};

use crate::{
    Device,
    video::{
        CGA_MEMORY_END, CGA_MEMORY_SIZE, CGA_MEMORY_START, EGA_MEMORY_END, EGA_MEMORY_START,
        EGA_PLANE_SIZE, Mode, VIDEO_MEMORY_SIZE, VideoBuffer, VideoCardType,
        font::{CHAR_HEIGHT_8, Cp437Font},
        mode::TextDimensions,
    },
};

// CGA/EGA/VGA CRTC ports
pub const VIDEO_CARD_CONTROL_ADDR: u16 = 0x03D4;
pub const VIDEO_CARD_DATA_ADDR: u16 = 0x03D5;
pub const CGA_MODE_CTRL_ADDR: u16 = 0x03D8;
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
    cga_mode_ctrl: u8,
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
    /// Set/Reset (GC register 0x00): per-plane constant value (0xFF or 0x00) used when enabled.
    gc_set_reset: u8,
    /// Enable Set/Reset (GC register 0x01): per-plane enable for set/reset override.
    gc_enable_set_reset: u8,
    /// Read Map Select (GC register 0x04): which plane (0-3) the CPU reads from.
    gc_read_map_select: u8,
    /// Data Rotate / Function Select (GC register 0x03):
    /// bits 2:0 = rotate count, bits 4:3 = ALU function (0=replace,1=AND,2=OR,3=XOR).
    gc_data_rotate: u8,
    gc_function_select: u8,
    /// Graphics Mode (GC register 0x05): bits 1:0 = write mode, bit 3 = read mode.
    gc_write_mode: u8,
    /// Bit Mask (GC register 0x08): bit N=1 means CPU data bit N passes through to VRAM.
    gc_bit_mask: u8,
    /// CPU read latches: one byte per plane, loaded on every EGA CPU read.
    gc_latches: Cell<[u8; 4]>,
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
            cga_mode_ctrl: 0,
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
            gc_set_reset: 0,
            gc_enable_set_reset: 0,
            gc_read_map_select: 0,
            gc_data_rotate: 0,
            gc_function_select: 0,
            gc_write_mode: 0,
            gc_bit_mask: 0xFF,
            gc_latches: Cell::new([0u8; 4]),
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

    /// Apply GC ALU function (gc_function_select) between src and latch.
    fn apply_gc_alu(&self, src: u8, latch: u8) -> u8 {
        match self.gc_function_select {
            1 => src & latch,
            2 => src | latch,
            3 => src ^ latch,
            _ => src, // 0 = replace
        }
    }

    pub(crate) fn set_mode(&mut self, mode: Mode) -> Option<TextDimensions> {
        log::info!("set mode: {mode}");
        let dims = mode.get_text_dimensions();
        self.buffer.write().unwrap().set_mode(mode);
        dims
    }

    /// Draw a character transparently into EGA planar VRAM.
    ///
    /// Foreground pixels (glyph bit = 1) are set to `fg_color` in all planes.
    /// Background pixels (glyph bit = 0) are left unchanged (transparent).
    ///
    /// `char_row` and `char_col` are character-cell coordinates.
    /// `bytes_per_row` is the number of bytes per pixel row (40 for mode 0Dh, 80 for mode 10h).
    /// `char_height` is the character cell height in pixels (8 for mode 0Dh, 14 for mode 10h).
    pub(crate) fn ega_draw_char_transparent(
        &self,
        ch: u8,
        char_row: u8,
        char_col: u8,
        fg_color: u8,
        bytes_per_row: usize,
        char_height: usize,
    ) {
        let font = Cp437Font::new();
        let glyph = if char_height <= CHAR_HEIGHT_8 {
            font.get_glyph_8(ch)
        } else {
            font.get_glyph_16(ch)
        };
        let mut buffer = self.buffer.write().unwrap();
        for (r, &glyph_byte) in glyph.iter().enumerate().take(char_height) {
            let pixel_y = char_row as usize * char_height + r;
            let byte_offset = pixel_y * bytes_per_row + char_col as usize;
            for plane in 0..4u8 {
                let plane_vram = plane as usize * EGA_PLANE_SIZE + byte_offset;
                if plane_vram >= buffer.vram_len() {
                    continue;
                }
                // Foreground pixels (glyph bit=1) → fg_color's plane bit.
                // Background pixels (glyph bit=0) → 0 (black), matching real BIOS AH=0Eh behavior.
                let plane_bit = (fg_color >> plane) & 1;
                let new_val = if plane_bit != 0 { glyph_byte } else { 0 };
                buffer.write_vram(plane_vram, new_val);
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
        self.cga_mode_ctrl = 0;
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
        self.gc_set_reset = 0;
        self.gc_enable_set_reset = 0;
        self.gc_read_map_select = 0;
        self.gc_data_rotate = 0;
        self.gc_function_select = 0;
        self.gc_write_mode = 0;
        self.gc_bit_mask = 0xFF;
        self.gc_latches.set([0u8; 4]);
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
                // Load latches from all 4 planes on every CPU read (hardware behaviour).
                let latches = [
                    self.internal_read_u8(offset),
                    self.internal_read_u8(EGA_PLANE_SIZE + offset),
                    self.internal_read_u8(2 * EGA_PLANE_SIZE + offset),
                    self.internal_read_u8(3 * EGA_PLANE_SIZE + offset),
                ];
                self.gc_latches.set(latches);
                Some(latches[self.gc_read_map_select as usize])
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
                let latches = self.gc_latches.get();
                // Rotate CPU data right by gc_data_rotate bits (write modes 0 and 3).
                let rotated = val.rotate_right(self.gc_data_rotate as u32);

                #[allow(clippy::needless_range_loop)]
                for plane in 0..4usize {
                    if (self.sequencer_map_mask >> plane) & 1 == 0 {
                        continue;
                    }
                    let latch = latches[plane];
                    let plane_offset = plane * EGA_PLANE_SIZE + offset;

                    let final_val = match self.gc_write_mode {
                        1 => {
                            // Mode 1: copy latch directly, no ALU or bit mask.
                            latch
                        }
                        2 => {
                            // Mode 2: CPU bits 3:0 are a 4-bit color; expand plane bit to 0x00/0xFF.
                            let src = if (val >> plane) & 1 != 0 { 0xFF } else { 0x00 };
                            let after_alu = self.apply_gc_alu(src, latch);
                            (after_alu & self.gc_bit_mask) | (latch & !self.gc_bit_mask)
                        }
                        // Mode 3 (VGA): AND rotated data with gc_bit_mask to form per-bit mask,
                        // then write set/reset value through that mask.
                        3 => {
                            let src = if (self.gc_set_reset >> plane) & 1 != 0 {
                                0xFF
                            } else {
                                0x00
                            };
                            let effective_mask = self.gc_bit_mask & rotated;
                            (src & effective_mask) | (latch & !effective_mask)
                        }
                        // Mode 0 (default): set/reset or rotated data → ALU → bit mask.
                        _ => {
                            let src = if (self.gc_enable_set_reset >> plane) & 1 != 0 {
                                if (self.gc_set_reset >> plane) & 1 != 0 {
                                    0xFF
                                } else {
                                    0x00
                                }
                            } else {
                                rotated
                            };
                            let after_alu = self.apply_gc_alu(src, latch);
                            (after_alu & self.gc_bit_mask) | (latch & !self.gc_bit_mask)
                        }
                    };
                    self.internal_write_u8(plane_offset, final_val);
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
                        VIDEO_CARD_REG_CURSOR_START_LINE => buffer.cursor_start_line(),
                        VIDEO_CARD_REG_CURSOR_END_LINE => buffer.cursor_end_line(),
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
                    0x00 => self.gc_set_reset,
                    0x01 => self.gc_enable_set_reset,
                    0x03 => self.gc_data_rotate | (self.gc_function_select << 3),
                    0x04 => self.gc_read_map_select,
                    0x05 => self.gc_write_mode,
                    0x08 => self.gc_bit_mask,
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
                CGA_MODE_CTRL_ADDR => {
                    self.cga_mode_ctrl = val;
                    true
                }
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
                            let visible = (val & 0x20) == 0;
                            let start = val & 0x1F;
                            log::debug!(
                                "CRTC reg 0x0A: cursor start={} visible={}",
                                start,
                                visible
                            );
                            buffer.set_cursor_visible(visible);
                            buffer.set_cursor_start_line(start);
                        }
                        VIDEO_CARD_REG_CURSOR_END_LINE => {
                            log::debug!("CRTC reg 0x0B: cursor end={}", val);
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
                        // CRTC timing registers (horizontal/vertical) — not needed by emulator
                        0x00..=0x09 => {}
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
                        0x00 => self.gc_set_reset = val & 0x0F,
                        0x01 => self.gc_enable_set_reset = val & 0x0F,
                        0x03 => {
                            self.gc_data_rotate = val & 0x07;
                            self.gc_function_select = (val >> 3) & 0x03;
                        }
                        0x04 => self.gc_read_map_select = val & 0x03,
                        0x05 => {
                            self.gc_write_mode = val & 0x03;
                            // bit 3 = read mode; not yet used
                        }
                        0x08 => self.gc_bit_mask = val,
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
