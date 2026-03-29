use std::{
    any::Any,
    sync::{Arc, RwLock},
};

use crate::{
    Device, byte_to_printable_char,
    video::{
        CGA_MEMORY_END, CGA_MEMORY_SIZE, CGA_MEMORY_START, EGA_MEMORY_END, EGA_MEMORY_START,
        EGA_PLANE_SIZE, MDA_MEMORY_END, MDA_MEMORY_SIZE, MDA_MEMORY_START, Mode,
        VGA_MODE_13_FRAMEBUFFER_SIZE, VGA_MODE_13_WIDTH, VIDEO_MEMORY_SIZE, VideoBuffer,
        VideoCardType,
        font::{CHAR_HEIGHT_8, Cp437Font},
        mode::TextDimensions,
        palette::{TextModePalette, VGA_DEFAULT_DAC_PALETTE},
    },
};

// CGA/EGA/VGA CRTC ports
pub const VIDEO_CARD_CONTROL_ADDR: u16 = 0x03D4;
pub const VIDEO_CARD_DATA_ADDR: u16 = 0x03D5;
pub const CGA_MODE_CTRL_ADDR: u16 = 0x03D8;
pub const CGA_COLOR_SELECT_ADDR: u16 = 0x03D9;

// MDA/HGC CRTC ports (same 6845 chip as CGA but at different addresses)
pub const MDA_CRTC_CONTROL_ADDR: u16 = 0x03B4;
pub const MDA_CRTC_DATA_ADDR: u16 = 0x03B5;
const MDA_MODE_CTRL_ADDR: u16 = 0x03B8; // Mode control register (write-only)
const MDA_STATUS_ADDR: u16 = 0x03BA; // Status register: bit0=hsync, bit7=vsync (HGC only)

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
/// CGA scanlines per frame (200 visible + 62 blanking).
const CGA_LINES_PER_FRAME: u64 = 262;
/// Horizontal retrace active for roughly 1/5 of each scanline (~20%).
const CGA_HSYNC_DUTY_DIVISOR: u64 = 5;

/// Parameters for drawing a pre-fetched glyph into EGA planar VRAM.
pub(crate) struct EgaGlyphParams<'a> {
    pub glyph: &'a [u8],
    pub char_row: u8,
    pub char_col: u8,
    pub fg_color: u8,
    pub bytes_per_row: usize,
    pub char_height: usize,
    /// If true, XOR the glyph onto existing VRAM instead of overwriting.
    pub xor: bool,
}

/// Window parameters shared by all BIOS scroll operations.
pub(crate) struct ScrollWindow {
    /// Number of lines to scroll (0 = clear entire window).
    pub lines: u8,
    pub top: u8,
    pub left: u8,
    pub bottom: u8,
    pub right: u8,
}

pub struct VideoCard {
    card_type: VideoCardType,
    buffer: Arc<RwLock<VideoBuffer>>,
    vram_size: usize,
    cpu_clock_speed: u32,
    io_register: u8,
    cga_mode_ctrl: u8,
    mda_mode_ctrl: u8,
    color_select: u8,
    // EGA/VGA Attribute Controller registers (16 palette + 1 border color)
    ac_registers: [u8; 17],
    ac_address: u8,
    ac_flip_flop: bool, // false = address mode, true = data mode
    // VGA DAC registers (256 entries, each RGB 0-63)
    dac_registers: Vec<[u8; 3]>,
    dac_write_pos: usize, // index * 3 + color_component
    dac_read_pos: usize,
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
    gc_latches: [u8; 4],
    /// VGA Miscellaneous Output Register (write: 0x3C2, read: 0x3CC).
    /// Default 0x67: color mode, RAM enabled, 25.175 MHz clock, positive syncs.
    misc_output: u8,
}

impl VideoCard {
    pub fn new(
        card_type: VideoCardType,
        buffer: Arc<RwLock<VideoBuffer>>,
        cpu_clock_speed: u32,
    ) -> Self {
        let dac_registers = match card_type {
            VideoCardType::VGA => VGA_DEFAULT_DAC_PALETTE.to_vec(),
            VideoCardType::EGA | VideoCardType::MDA | VideoCardType::HGC => {
                let mut regs = vec![[0u8; 3]; 256];
                for (i, entry) in regs.iter_mut().enumerate().take(16) {
                    *entry = TextModePalette::get_dac_color(i as u8);
                }
                regs
            }
            VideoCardType::CGA => vec![[0u8; 3]; 256],
        };
        Self {
            card_type,
            buffer,
            vram_size: match card_type {
                VideoCardType::EGA | VideoCardType::VGA => VIDEO_MEMORY_SIZE,
                VideoCardType::MDA | VideoCardType::HGC => MDA_MEMORY_SIZE,
                VideoCardType::CGA => CGA_MEMORY_SIZE,
            },
            cpu_clock_speed,
            io_register: 0,
            cga_mode_ctrl: 0,
            mda_mode_ctrl: 0,
            color_select: 0,
            ac_registers: [0u8; 17],
            ac_address: 0,
            ac_flip_flop: false,
            dac_registers,
            dac_write_pos: 0,
            dac_read_pos: 0,
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
            gc_latches: [0u8; 4],
            misc_output: 0x67,
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
        let mut buffer = self.buffer.write().unwrap();
        buffer.reset_crtc_overrides();
        buffer.set_mode(mode);
        // Re-initialize DAC registers with the default palette for this card type so
        // programs that read them back (e.g. via INT 10h/AL=0x17) get correct values.
        match self.card_type {
            VideoCardType::VGA => {
                for (i, &entry) in VGA_DEFAULT_DAC_PALETTE.iter().enumerate() {
                    self.dac_registers[i] = entry;
                    buffer.set_dac_color(i, entry[0], entry[1], entry[2]);
                }
            }
            VideoCardType::EGA => {
                for (i, entry) in self.dac_registers.iter_mut().enumerate().take(16) {
                    let [r, g, b] = TextModePalette::get_dac_color(i as u8);
                    *entry = [r, g, b];
                    buffer.set_dac_color(i, r, g, b);
                }
            }
            VideoCardType::CGA | VideoCardType::MDA | VideoCardType::HGC => {}
        }
        dims
    }

    /// Scroll up or down a rectangular window in text-mode VRAM.
    ///
    /// Coordinates are in character cells. `lines == 0` clears the window.
    /// Blank rows are filled with space (0x20) and `attr`. `cols` is the total
    /// screen width used for offset calculation and coordinate clamping.
    pub(crate) fn scroll_text_window(
        &mut self,
        w: ScrollWindow,
        cols: u8,
        rows: u8,
        attr: u8,
        scroll_down: bool,
    ) {
        let ScrollWindow {
            lines,
            top,
            left,
            bottom,
            right,
        } = w;
        let right = right.min(cols.saturating_sub(1));
        let bottom = bottom.min(rows);
        if top > bottom || left > right {
            return;
        }
        let stride = cols as usize;
        if lines == 0 {
            for row in top..=bottom {
                for col in left..=right {
                    let off = (row as usize * stride + col as usize) * 2;
                    self.internal_write_u8(off, b' ');
                    self.internal_write_u8(off + 1, attr);
                }
            }
        } else if scroll_down {
            for row in (top..=bottom).rev() {
                for col in left..=right {
                    let dest = (row as usize * stride + col as usize) * 2;
                    if row >= top + lines {
                        let src = ((row - lines) as usize * stride + col as usize) * 2;
                        let ch = self.internal_read_u8(src);
                        let at = self.internal_read_u8(src + 1);
                        self.internal_write_u8(dest, ch);
                        self.internal_write_u8(dest + 1, at);
                    } else {
                        self.internal_write_u8(dest, b' ');
                        self.internal_write_u8(dest + 1, attr);
                    }
                }
            }
        } else {
            for row in top..=bottom {
                for col in left..=right {
                    let dest = (row as usize * stride + col as usize) * 2;
                    let src_row = row + lines;
                    if src_row <= bottom {
                        let src = (src_row as usize * stride + col as usize) * 2;
                        let ch = self.internal_read_u8(src);
                        let at = self.internal_read_u8(src + 1);
                        self.internal_write_u8(dest, ch);
                        self.internal_write_u8(dest + 1, at);
                    } else {
                        self.internal_write_u8(dest, b' ');
                        self.internal_write_u8(dest + 1, attr);
                    }
                }
            }
        }
    }

    /// Scroll up or down a rectangular window in CGA interleaved VRAM.
    ///
    /// Coordinates are in character cells. `lines == 0` clears the window.
    /// `bytes_per_col` is 1 for mode 06h (1bpp) and 2 for mode 04h (2bpp).
    /// Blank rows are cleared to 0.
    pub(crate) fn cga_scroll_window(
        &mut self,
        w: ScrollWindow,
        bytes_per_col: usize,
        scroll_down: bool,
    ) {
        let ScrollWindow {
            lines,
            top,
            left,
            bottom,
            right,
        } = w;
        if lines == 0 {
            for char_row in top..=bottom {
                for pr in 0..CHAR_HEIGHT_8 {
                    let pixel_y = char_row as usize * CHAR_HEIGHT_8 + pr;
                    let bank = if pixel_y % 2 == 1 { 0x2000 } else { 0 };
                    let row_start = bank + (pixel_y / 2) * 80;
                    for col in left..=right {
                        let byte_start = row_start + col as usize * bytes_per_col;
                        for b in 0..bytes_per_col {
                            self.internal_write_u8(byte_start + b, 0);
                        }
                    }
                }
            }
            return;
        }

        if scroll_down {
            for char_row in (top..=bottom).rev() {
                for pr in 0..CHAR_HEIGHT_8 {
                    let dest_pixel_y = char_row as usize * CHAR_HEIGHT_8 + pr;
                    let dest_bank = if dest_pixel_y % 2 == 1 { 0x2000 } else { 0 };
                    let dest_row_start = dest_bank + (dest_pixel_y / 2) * 80;
                    for col in left..=right {
                        let dest_byte_start = dest_row_start + col as usize * bytes_per_col;
                        if char_row >= top + lines {
                            let src_char_row = char_row - lines;
                            let src_pixel_y = src_char_row as usize * CHAR_HEIGHT_8 + pr;
                            let src_bank = if src_pixel_y % 2 == 1 { 0x2000 } else { 0 };
                            let src_row_start = src_bank + (src_pixel_y / 2) * 80;
                            let src_byte_start = src_row_start + col as usize * bytes_per_col;
                            for b in 0..bytes_per_col {
                                let v = self.internal_read_u8(src_byte_start + b);
                                self.internal_write_u8(dest_byte_start + b, v);
                            }
                        } else {
                            for b in 0..bytes_per_col {
                                self.internal_write_u8(dest_byte_start + b, 0);
                            }
                        }
                    }
                }
            }
        } else {
            for char_row in top..=bottom {
                let src_char_row = char_row + lines;
                for pr in 0..CHAR_HEIGHT_8 {
                    let dest_pixel_y = char_row as usize * CHAR_HEIGHT_8 + pr;
                    let dest_bank = if dest_pixel_y % 2 == 1 { 0x2000 } else { 0 };
                    let dest_row_start = dest_bank + (dest_pixel_y / 2) * 80;
                    for col in left..=right {
                        let dest_byte_start = dest_row_start + col as usize * bytes_per_col;
                        if src_char_row <= bottom {
                            let src_pixel_y = src_char_row as usize * CHAR_HEIGHT_8 + pr;
                            let src_bank = if src_pixel_y % 2 == 1 { 0x2000 } else { 0 };
                            let src_row_start = src_bank + (src_pixel_y / 2) * 80;
                            let src_byte_start = src_row_start + col as usize * bytes_per_col;
                            for b in 0..bytes_per_col {
                                let v = self.internal_read_u8(src_byte_start + b);
                                self.internal_write_u8(dest_byte_start + b, v);
                            }
                        } else {
                            for b in 0..bytes_per_col {
                                self.internal_write_u8(dest_byte_start + b, 0);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Scroll up a rectangular window in EGA planar VRAM.
    ///
    /// Coordinates are in character cells. `lines == 0` clears the window.
    /// Cleared/blank rows are filled with `fill_color` (EGA color 0-15) in all planes.
    pub(crate) fn ega_scroll_up_window(
        &self,
        w: ScrollWindow,
        bytes_per_row: usize,
        char_height: usize,
        fill_color: u8,
    ) {
        let ScrollWindow {
            lines,
            top,
            left,
            bottom,
            right,
        } = w;
        let fill_bytes: [u8; 4] = [
            if fill_color & 1 != 0 { 0xFF } else { 0x00 },
            if (fill_color >> 1) & 1 != 0 {
                0xFF
            } else {
                0x00
            },
            if (fill_color >> 2) & 1 != 0 {
                0xFF
            } else {
                0x00
            },
            if (fill_color >> 3) & 1 != 0 {
                0xFF
            } else {
                0x00
            },
        ];
        let mut buffer = self.buffer.write().unwrap();
        if lines == 0 {
            for char_row in top..=bottom {
                for pr in 0..char_height {
                    let pixel_y = char_row as usize * char_height + pr;
                    let row_start = pixel_y * bytes_per_row;
                    for col in left..=right {
                        let byte_off = row_start + col as usize;
                        for (plane, &fill_byte) in fill_bytes.iter().enumerate() {
                            let vram_off = plane * EGA_PLANE_SIZE + byte_off;
                            if vram_off < buffer.vram_len() {
                                buffer.write_vram(vram_off, fill_byte);
                            }
                        }
                    }
                }
            }
        } else {
            for char_row in top..=bottom {
                let src_char_row = char_row + lines;
                for pr in 0..char_height {
                    let dest_pixel_y = char_row as usize * char_height + pr;
                    let dest_row_start = dest_pixel_y * bytes_per_row;
                    for col in left..=right {
                        let dest_off = dest_row_start + col as usize;
                        if src_char_row <= bottom {
                            let src_pixel_y = src_char_row as usize * char_height + pr;
                            let src_off = src_pixel_y * bytes_per_row + col as usize;
                            for plane in 0..4usize {
                                let sv = plane * EGA_PLANE_SIZE + src_off;
                                let dv = plane * EGA_PLANE_SIZE + dest_off;
                                if sv < buffer.vram_len() && dv < buffer.vram_len() {
                                    let v = buffer.read_vram(sv);
                                    buffer.write_vram(dv, v);
                                }
                            }
                        } else {
                            for (plane, &fill_byte) in fill_bytes.iter().enumerate() {
                                let dv = plane * EGA_PLANE_SIZE + dest_off;
                                if dv < buffer.vram_len() {
                                    buffer.write_vram(dv, fill_byte);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Scroll down a rectangular window in EGA planar VRAM.
    ///
    /// Coordinates are in character cells. `lines == 0` clears the window.
    /// Cleared/blank rows are filled with `fill_color` (EGA color 0-15) in all planes.
    pub(crate) fn ega_scroll_down_window(
        &self,
        w: ScrollWindow,
        bytes_per_row: usize,
        char_height: usize,
        fill_color: u8,
    ) {
        let ScrollWindow {
            lines,
            top,
            left,
            bottom,
            right,
        } = w;
        let fill_bytes: [u8; 4] = [
            if fill_color & 1 != 0 { 0xFF } else { 0x00 },
            if (fill_color >> 1) & 1 != 0 {
                0xFF
            } else {
                0x00
            },
            if (fill_color >> 2) & 1 != 0 {
                0xFF
            } else {
                0x00
            },
            if (fill_color >> 3) & 1 != 0 {
                0xFF
            } else {
                0x00
            },
        ];
        let mut buffer = self.buffer.write().unwrap();
        if lines == 0 {
            for char_row in top..=bottom {
                for pr in 0..char_height {
                    let pixel_y = char_row as usize * char_height + pr;
                    let row_start = pixel_y * bytes_per_row;
                    for col in left..=right {
                        let byte_off = row_start + col as usize;
                        for (plane, &fill_byte) in fill_bytes.iter().enumerate() {
                            let vram_off = plane * EGA_PLANE_SIZE + byte_off;
                            if vram_off < buffer.vram_len() {
                                buffer.write_vram(vram_off, fill_byte);
                            }
                        }
                    }
                }
            }
        } else {
            for char_row in (top..=bottom).rev() {
                for pr in 0..char_height {
                    let dest_pixel_y = char_row as usize * char_height + pr;
                    let dest_row_start = dest_pixel_y * bytes_per_row;
                    for col in left..=right {
                        let dest_off = dest_row_start + col as usize;
                        if char_row >= top + lines {
                            let src_char_row = char_row - lines;
                            let src_pixel_y = src_char_row as usize * char_height + pr;
                            let src_off = src_pixel_y * bytes_per_row + col as usize;
                            for plane in 0..4usize {
                                let sv = plane * EGA_PLANE_SIZE + src_off;
                                let dv = plane * EGA_PLANE_SIZE + dest_off;
                                if sv < buffer.vram_len() && dv < buffer.vram_len() {
                                    let v = buffer.read_vram(sv);
                                    buffer.write_vram(dv, v);
                                }
                            }
                        } else {
                            for (plane, &fill_byte) in fill_bytes.iter().enumerate() {
                                let dv = plane * EGA_PLANE_SIZE + dest_off;
                                if dv < buffer.vram_len() {
                                    buffer.write_vram(dv, fill_byte);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Scroll up a rectangular window in VGA mode 13h linear VRAM (320x200, 1 byte/pixel).
    ///
    /// Coordinates are in character cells (8x8 pixels each). `lines == 0` clears the window.
    /// Blank rows are filled with `fill_color` (palette index).
    pub(crate) fn vga_scroll_up_window(&self, w: ScrollWindow, fill_color: u8) {
        let ScrollWindow {
            lines,
            top,
            left,
            bottom,
            right,
        } = w;
        let mut buffer = self.buffer.write().unwrap();
        if lines == 0 {
            for char_row in top..=bottom {
                for pr in 0..CHAR_HEIGHT_8 {
                    let pixel_y = char_row as usize * CHAR_HEIGHT_8 + pr;
                    for col in left..=right {
                        for b in 0..8usize {
                            let off = pixel_y * VGA_MODE_13_WIDTH + col as usize * 8 + b;
                            if off < VGA_MODE_13_FRAMEBUFFER_SIZE {
                                buffer.write_vram(off, fill_color);
                            }
                        }
                    }
                }
            }
        } else {
            let lines = lines as usize;
            for char_row in top as usize..=bottom as usize {
                for pr in 0..CHAR_HEIGHT_8 {
                    let dest_pixel_y = char_row * CHAR_HEIGHT_8 + pr;
                    for col in left..=right {
                        for b in 0..8usize {
                            let pixel_x = col as usize * 8 + b;
                            let dest_off = dest_pixel_y * VGA_MODE_13_WIDTH + pixel_x;
                            if dest_off >= VGA_MODE_13_FRAMEBUFFER_SIZE {
                                continue;
                            }
                            let src_char_row = char_row + lines;
                            if src_char_row <= bottom as usize {
                                let src_off = (src_char_row * CHAR_HEIGHT_8 + pr)
                                    * VGA_MODE_13_WIDTH
                                    + pixel_x;
                                if src_off < VGA_MODE_13_FRAMEBUFFER_SIZE {
                                    let v = buffer.read_vram(src_off);
                                    buffer.write_vram(dest_off, v);
                                }
                            } else {
                                buffer.write_vram(dest_off, fill_color);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Scroll down a rectangular window in VGA mode 13h linear VRAM (320x200, 1 byte/pixel).
    ///
    /// Coordinates are in character cells (8x8 pixels each). `lines == 0` clears the window.
    /// Blank rows are filled with `fill_color` (palette index).
    pub(crate) fn vga_scroll_down_window(&self, w: ScrollWindow, fill_color: u8) {
        let ScrollWindow {
            lines,
            top,
            left,
            bottom,
            right,
        } = w;
        let mut buffer = self.buffer.write().unwrap();
        if lines == 0 {
            for char_row in top..=bottom {
                for pr in 0..CHAR_HEIGHT_8 {
                    let pixel_y = char_row as usize * CHAR_HEIGHT_8 + pr;
                    for col in left..=right {
                        for b in 0..8usize {
                            let off = pixel_y * VGA_MODE_13_WIDTH + col as usize * 8 + b;
                            if off < VGA_MODE_13_FRAMEBUFFER_SIZE {
                                buffer.write_vram(off, fill_color);
                            }
                        }
                    }
                }
            }
        } else {
            let lines = lines as usize;
            for char_row in (top as usize..=bottom as usize).rev() {
                for pr in 0..CHAR_HEIGHT_8 {
                    let dest_pixel_y = char_row * CHAR_HEIGHT_8 + pr;
                    for col in left..=right {
                        for b in 0..8usize {
                            let pixel_x = col as usize * 8 + b;
                            let dest_off = dest_pixel_y * VGA_MODE_13_WIDTH + pixel_x;
                            if dest_off >= VGA_MODE_13_FRAMEBUFFER_SIZE {
                                continue;
                            }
                            if char_row >= top as usize + lines {
                                let src_char_row = char_row - lines;
                                let src_off = (src_char_row * CHAR_HEIGHT_8 + pr)
                                    * VGA_MODE_13_WIDTH
                                    + pixel_x;
                                if src_off < VGA_MODE_13_FRAMEBUFFER_SIZE {
                                    let v = buffer.read_vram(src_off);
                                    buffer.write_vram(dest_off, v);
                                }
                            } else {
                                buffer.write_vram(dest_off, fill_color);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Draw a character into EGA planar VRAM.
    ///
    /// Foreground pixels (glyph bit = 1) are set to `fg_color` in all planes.
    /// Background pixels (glyph bit = 0) are set to 0 (black).
    ///
    /// `char_row` and `char_col` are character-cell coordinates.
    /// `bytes_per_row` is the number of bytes per pixel row (40 for mode 0Dh, 80 for mode 10h).
    /// `char_height` is the character cell height in pixels (8 for mode 0Dh, 14 for mode 10h).
    pub(crate) fn ega_draw_char(
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
        self.ega_draw_glyph(EgaGlyphParams {
            glyph,
            char_row,
            char_col,
            fg_color,
            bytes_per_row,
            char_height,
            xor: false,
        });
    }

    /// Draw a pre-fetched glyph into EGA planar VRAM.
    ///
    /// Foreground pixels (glyph bit = 1) are set to `fg_color` in all planes.
    /// Background pixels (glyph bit = 0) are set to 0 (black).
    pub(crate) fn ega_draw_glyph(&self, params: EgaGlyphParams<'_>) {
        let EgaGlyphParams {
            glyph,
            char_row,
            char_col,
            fg_color,
            bytes_per_row,
            char_height,
            xor,
        } = params;
        let mut buffer = self.buffer.write().unwrap();
        for (r, &glyph_byte) in glyph.iter().enumerate().take(char_height) {
            let pixel_y = char_row as usize * char_height + r;
            let byte_offset = pixel_y * bytes_per_row + char_col as usize;
            for plane in 0..4u8 {
                let plane_vram = plane as usize * EGA_PLANE_SIZE + byte_offset;
                if plane_vram >= buffer.vram_len() {
                    continue;
                }
                let plane_bit = (fg_color >> plane) & 1;
                let glyph_plane = if plane_bit != 0 { glyph_byte } else { 0 };
                let new_val = if xor {
                    buffer.read_vram(plane_vram) ^ glyph_plane
                } else {
                    glyph_plane
                };
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
            VideoCardType::MDA | VideoCardType::HGC => MDA_MEMORY_SIZE,
            VideoCardType::CGA => CGA_MEMORY_SIZE,
        };
        self.io_register = 0;
        self.cga_mode_ctrl = 0;
        self.mda_mode_ctrl = 0;
        self.color_select = 0;
        self.ac_registers = [0u8; 17];
        self.ac_address = 0;
        self.ac_flip_flop = false;
        match self.card_type {
            VideoCardType::VGA => self.dac_registers.copy_from_slice(&VGA_DEFAULT_DAC_PALETTE),
            VideoCardType::EGA | VideoCardType::MDA | VideoCardType::HGC => {
                for (i, entry) in self.dac_registers.iter_mut().enumerate().take(16) {
                    *entry = TextModePalette::get_dac_color(i as u8);
                }
                for entry in self.dac_registers[16..].iter_mut() {
                    *entry = [0u8; 3];
                }
            }
            VideoCardType::CGA => {
                for entry in self.dac_registers.iter_mut() {
                    *entry = [0u8; 3];
                }
            }
        }
        self.dac_write_pos = 0;
        self.dac_read_pos = 0;
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
        self.gc_latches = [0u8; 4];
        self.misc_output = 0x67;
    }

    fn memory_read_u8(&mut self, addr: usize, _cycle_count: u32) -> Option<u8> {
        if matches!(self.card_type, VideoCardType::MDA | VideoCardType::HGC)
            && (MDA_MEMORY_START..=MDA_MEMORY_END).contains(&addr)
        {
            Some(self.internal_read_u8(addr - MDA_MEMORY_START))
        } else if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&addr) {
            let offset = addr - CGA_MEMORY_START;
            Some(self.internal_read_u8(offset))
        } else if matches!(self.card_type, VideoCardType::EGA | VideoCardType::VGA)
            && (EGA_MEMORY_START..=EGA_MEMORY_END).contains(&addr)
        {
            let offset = addr - EGA_MEMORY_START;
            let is_mode_13 = {
                let buf = self.buffer.read().unwrap();
                matches!(buf.mode(), Mode::M13Vga320x200x256)
            };
            if is_mode_13 {
                // Linear framebuffer: direct byte read
                if offset < VGA_MODE_13_FRAMEBUFFER_SIZE {
                    Some(self.internal_read_u8(offset))
                } else {
                    Some(0xFF)
                }
            } else if offset < EGA_PLANE_SIZE {
                // Load latches from all 4 planes on every CPU read (hardware behaviour).
                let latches = [
                    self.internal_read_u8(offset),
                    self.internal_read_u8(EGA_PLANE_SIZE + offset),
                    self.internal_read_u8(2 * EGA_PLANE_SIZE + offset),
                    self.internal_read_u8(3 * EGA_PLANE_SIZE + offset),
                ];
                self.gc_latches = latches;
                Some(latches[self.gc_read_map_select as usize])
            } else {
                Some(0xFF)
            }
        } else {
            None
        }
    }

    fn memory_write_u8(&mut self, addr: usize, val: u8, _cycle_count: u32) -> bool {
        if matches!(self.card_type, VideoCardType::MDA | VideoCardType::HGC)
            && (MDA_MEMORY_START..=MDA_MEMORY_END).contains(&addr)
        {
            self.internal_write_u8(addr - MDA_MEMORY_START, val);
            true
        } else if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&addr) {
            let offset = addr - CGA_MEMORY_START;
            if offset.is_multiple_of(2) {
                log::debug!(
                    "CGA text write char='{}' 0x{:02X} offset={:#x}",
                    byte_to_printable_char(val),
                    val,
                    offset,
                );
            }
            self.internal_write_u8(offset, val);
            true
        } else if matches!(self.card_type, VideoCardType::EGA | VideoCardType::VGA)
            && (EGA_MEMORY_START..=EGA_MEMORY_END).contains(&addr)
        {
            let offset = addr - EGA_MEMORY_START;
            let is_mode_13 = {
                let buf = self.buffer.read().unwrap();
                matches!(buf.mode(), Mode::M13Vga320x200x256)
            };
            if is_mode_13 {
                // Linear framebuffer: direct byte write
                if offset < VGA_MODE_13_FRAMEBUFFER_SIZE {
                    self.internal_write_u8(offset, val);
                }
                return true;
            }
            if offset < EGA_PLANE_SIZE {
                let latches = self.gc_latches;
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

    fn io_read_u8(&mut self, port: u16, cycle_count: u32) -> Option<u8> {
        let is_ega_vga = matches!(self.card_type, VideoCardType::EGA | VideoCardType::VGA);
        match self.card_type {
            VideoCardType::MDA | VideoCardType::HGC => match port {
                MDA_CRTC_CONTROL_ADDR => Some(self.io_register),
                MDA_CRTC_DATA_ADDR => {
                    let buffer = self.buffer.read().unwrap();
                    Some(match self.io_register {
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
                    })
                }
                MDA_MODE_CTRL_ADDR => Some(self.mda_mode_ctrl),
                MDA_STATUS_ADDR => {
                    // Bit 0: horizontal retrace (simulated).
                    // Bit 7: vertical sync — toggles on HGC (used by findmono to distinguish
                    //        HGC from MDA), static 0 on MDA.
                    let cycles_per_frame = self.cpu_clock_speed as u64 / CGA_VSYNC_HZ;
                    let vsync_cycles = cycles_per_frame / CGA_VSYNC_DUTY_DIVISOR;
                    let phase = cycle_count as u64 % cycles_per_frame;
                    let in_vsync = phase < vsync_cycles;
                    let vsync_bit: u8 = if matches!(self.card_type, VideoCardType::HGC) && in_vsync
                    {
                        0x80
                    } else {
                        0x00
                    };
                    Some(vsync_bit)
                }
                _ => None,
            },
            VideoCardType::CGA | VideoCardType::EGA | VideoCardType::VGA => match port {
                // MDA ports: return 0xFF — no MDA card present
                MDA_CRTC_CONTROL_ADDR | MDA_CRTC_DATA_ADDR => Some(0xFF),
                // CRTC address register: return currently selected register index
                VIDEO_CARD_CONTROL_ADDR => Some(self.io_register),
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
                CGA_MODE_CTRL_ADDR => Some(self.cga_mode_ctrl),
                CGA_COLOR_SELECT_ADDR => Some(self.color_select),
                // Miscellaneous Output Register read (EGA/VGA only)
                0x3CC if is_ega_vga => Some(self.misc_output),
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
                    let pos = self.dac_read_pos;
                    let reg = pos / 3;
                    let component = pos % 3;
                    self.dac_read_pos = (pos + 1) % (256 * 3);
                    Some(self.dac_registers[reg][component])
                }
                // Input Status Register 1: resets AC flip-flop to address mode.
                // Bit 3: vertical retrace (vsync) active. Simulated at ~60Hz using cycle count.
                0x3DA => {
                    self.ac_flip_flop = false;
                    let cycles_per_frame = self.cpu_clock_speed as u64 / CGA_VSYNC_HZ;
                    let vsync_cycles = cycles_per_frame / CGA_VSYNC_DUTY_DIVISOR;
                    let phase = cycle_count as u64 % cycles_per_frame;
                    let in_vsync = phase < vsync_cycles;
                    // Bit 0: horizontal retrace (~20% duty cycle at ~15.7 kHz)
                    let cycles_per_line = cycles_per_frame / CGA_LINES_PER_FRAME;
                    let hsync_cycles = cycles_per_line / CGA_HSYNC_DUTY_DIVISOR;
                    let hphase = cycle_count as u64 % cycles_per_line;
                    let in_hsync = hphase < hsync_cycles;
                    let mut status = 0u8;
                    if in_vsync {
                        status |= 0x08;
                    }
                    if in_hsync {
                        status |= 0x01;
                    }
                    Some(status)
                }
                _ => None,
            },
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        let is_ega_vga = matches!(self.card_type, VideoCardType::EGA | VideoCardType::VGA);
        match self.card_type {
            VideoCardType::MDA | VideoCardType::HGC => match port {
                MDA_CRTC_CONTROL_ADDR => {
                    self.io_register = val;
                    true
                }
                MDA_CRTC_DATA_ADDR => {
                    let mut buffer = self.buffer.write().unwrap();
                    match self.io_register {
                        VIDEO_CARD_REG_CURSOR_START_LINE => {
                            buffer.set_cursor_visible((val & 0x20) == 0);
                            buffer.set_cursor_start_line(val & 0x1F);
                        }
                        VIDEO_CARD_REG_CURSOR_END_LINE => buffer.set_cursor_end_line(val & 0x1F),
                        VIDEO_CARD_REG_CURSOR_LOC_HIGH => {
                            let loc = (buffer.cursor_loc() & 0x00FF) | ((val as u16) << 8);
                            buffer.set_cursor_loc(loc);
                        }
                        VIDEO_CARD_REG_CURSOR_LOC_LOW => {
                            let loc = (buffer.cursor_loc() & 0xFF00) | val as u16;
                            buffer.set_cursor_loc(loc);
                        }
                        VIDEO_CARD_START_ADDRESS_HIGH_REGISTER => {
                            let addr = (buffer.start_address() & 0x00FF) | ((val as u16) << 8);
                            buffer.set_start_address(addr);
                        }
                        VIDEO_CARD_START_ADDRESS_LOW_REGISTER => {
                            let addr = (buffer.start_address() & 0xFF00) | val as u16;
                            buffer.set_start_address(addr);
                        }
                        _ => {}
                    }
                    true
                }
                MDA_MODE_CTRL_ADDR => {
                    self.mda_mode_ctrl = val;
                    if matches!(self.card_type, VideoCardType::HGC) && (val & 0x02 != 0) {
                        log::warn!(
                            "HGC graphics mode (0x3B8=0x{:02X}) not yet implemented",
                            val
                        );
                    }
                    true
                }
                MDA_STATUS_ADDR => true, // write-only register, ignore reads-as-write
                _ => false,
            },
            VideoCardType::CGA | VideoCardType::EGA | VideoCardType::VGA => match port {
                // MDA ports: silently ignore — no MDA card present
                MDA_CRTC_CONTROL_ADDR | MDA_CRTC_DATA_ADDR => true,
                CGA_MODE_CTRL_ADDR => {
                    self.cga_mode_ctrl = val;
                    // Derive the CGA rendering mode from the hardware register bits:
                    //   bit 1 = graphics mode enable
                    //   bit 3 = colorburst enable (composite output)
                    //   bit 4 = high-resolution (640x200) mode
                    //   bit 0 = 80-column (text mode only)
                    let mode = if val & 0x02 != 0 {
                        if val & 0x10 != 0 {
                            Mode::M06Cga640x200x2
                        } else {
                            Mode::M04Cga320x200x4
                        }
                    } else if val & 0x01 != 0 {
                        Mode::M03Text
                    } else {
                        Mode::M00ColorText40
                    };
                    let composite = (val & 0x08) != 0;
                    log::info!(
                        "CGA mode control register 0x3D8 = 0x{:02X} → mode {}, composite={}",
                        val,
                        mode,
                        composite
                    );
                    let mut buffer = self.buffer.write().unwrap();
                    buffer.set_mode(mode);
                    buffer.set_cga_composite(composite);
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
                        0x06 => {
                            log::debug!("CRTC reg 0x06: vertical displayed={}", val);
                            buffer.set_crtc_vertical_displayed(val);
                        }
                        0x09 => {
                            log::debug!("CRTC reg 0x09: max scan line={}", val);
                            buffer.set_crtc_max_scan_line(val);
                        }
                        0x13 => {
                            log::debug!(
                                "CRTC reg 0x13: offset={} ({} bytes/row)",
                                val,
                                val as usize * 2
                            );
                            buffer.set_crtc_offset(val);
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
                // Miscellaneous Output Register write (EGA/VGA only)
                0x3C2 if is_ega_vga => {
                    self.misc_output = val;
                    true
                }
                // AC address/data write (EGA/VGA only) — flip-flop toggles address vs data
                0x3C0 if is_ega_vga => {
                    if !self.ac_flip_flop {
                        self.ac_address = val & 0x1F;
                        self.ac_flip_flop = true;
                    } else {
                        let index = (self.ac_address & 0x0F) as usize;
                        self.ac_registers[index] = val;
                        // Sync AC palette registers 0-15 to the video buffer so the
                        // renderer can use them for CGA 4-color pixel → DAC resolution.
                        if self.ac_address < 0x10 {
                            self.buffer
                                .write()
                                .unwrap()
                                .set_ac_palette_register(index, val);
                        }
                        self.ac_flip_flop = false;
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
                    self.dac_read_pos = (val as usize) * 3;
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
                    // Sync completed entry to video buffer palette used by the renderer
                    if component == 2 {
                        let entry = self.dac_registers[reg];
                        log::debug!(
                            "DAC[{}] = RGB({}, {}, {})",
                            reg,
                            entry[0],
                            entry[1],
                            entry[2]
                        );
                        self.buffer
                            .write()
                            .unwrap()
                            .set_dac_color(reg, entry[0], entry[1], entry[2]);
                    }
                    true
                }
                _ => false,
            },
        }
    }
}
