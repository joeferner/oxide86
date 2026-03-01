use crate::video::font::{CHAR_HEIGHT, CHAR_WIDTH, Cp437Font};
use crate::video::palette::TextModePalette;
use crate::video::renderer::{RenderTextArgs, render_text};
use crate::video::text::TextAttribute;
use crate::video::{TEXT_MODE_COLS, TEXT_MODE_ROWS, TEXT_MODE_SIZE, VIDEO_MEMORY_SIZE};

#[derive(PartialEq)]
pub struct RenderResult {
    /// RGBA data
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Video mode type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoMode {
    /// Text modes: 80x25 or 40x25
    Text { cols: usize, rows: usize },
    /// CGA 320x200, 4 colors
    Graphics320x200,
    /// CGA 640x200, 2 colors
    Graphics640x200,
    /// EGA 320x200, 16 colors (mode 0x0D)
    Graphics320x200x16,
    /// VGA 320x200, 256 colors (mode 0x13) — linear framebuffer, 1 byte per pixel
    Graphics320x200x256,
}

/// Cursor position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorPosition {
    pub row: u8,
    pub col: u8,
}

pub struct VideoBuffer {
    mode: VideoMode,
    text_columns: u8,

    /// Raw video RAM (64KB).
    /// In CGA/text modes: framebuffer at B8000-BFFFF.
    /// In EGA mode 0x0D: 4 planes × EGA_PLANE_SIZE bytes (plane N at vram[N*EGA_PLANE_SIZE..]).
    /// In VGA mode 0x13: linear framebuffer vram[0..64000], 1 byte per pixel.
    /// Persists across mode changes, just like real hardware.
    vram: Vec<u8>,

    font: Cp437Font,
    /// VGA DAC palette registers (256 entries, each with 6-bit RGB components)
    vga_dac_palette: [[u8; 3]; 256],
    /// Blink/intensity mode for text attribute bit 7.
    /// true  = bit 7 enables character blinking (8 background colors, default)
    /// false = bit 7 selects high-intensity background (16 background colors, no blink)
    blink_enabled: bool,
    /// Cursor position as a character cell index, written by CRT controller
    /// registers 0x0E (high byte) and 0x0F (low byte). Row-major within the
    /// current text mode grid: col = loc % cols, row = loc / cols.
    cursor_loc: u16,
    /// If any value changes in the struct which could result in different output this will be set to true
    dirty: bool,
}

impl VideoBuffer {
    pub fn new() -> Self {
        let mut vram = vec![0; VIDEO_MEMORY_SIZE];
        for i in (0..TEXT_MODE_SIZE).step_by(2) {
            vram[i] = 0x20; // space
            vram[i + 1] = 0x07; // Light Gray on Black
        }
        Self {
            mode: VideoMode::Text {
                cols: TEXT_MODE_COLS,
                rows: TEXT_MODE_ROWS,
            },
            text_columns: TEXT_MODE_COLS as u8,
            vram,
            font: Cp437Font::new(),
            vga_dac_palette: Self::default_vga_dac_palette(),
            blink_enabled: false,
            cursor_loc: 0,
            dirty: false,
        }
    }

    /// Initialize VGA DAC palette with EGA defaults
    fn default_vga_dac_palette() -> [[u8; 3]; 256] {
        let mut palette = [[0u8; 3]; 256];
        // Initialize first 16 colors with EGA defaults (6-bit RGB values 0-63)
        for (i, entry) in palette.iter_mut().enumerate().take(16) {
            *entry = TextModePalette::get_dac_color(i as u8);
        }
        palette
    }

    pub fn mode(&self) -> VideoMode {
        self.mode
    }

    pub fn read_vram(&self, addr: usize) -> u8 {
        self.vram[addr]
    }

    pub fn write_vram(&mut self, addr: usize, val: u8) {
        self.vram[addr] = val;
        self.dirty = true;
    }

    pub fn cursor_loc(&self) -> u16 {
        self.cursor_loc
    }

    pub fn set_cursor_loc(&mut self, loc: u16) {
        self.cursor_loc = loc;
        self.dirty = true;
    }

    pub fn get_cursor_position(&self) -> CursorPosition {
        CursorPosition {
            row: (self.cursor_loc / self.text_columns as u16) as u8,
            col: (self.cursor_loc % self.text_columns as u16) as u8,
        }
    }

    pub fn blink_enabled(&self) -> bool {
        self.blink_enabled
    }

    pub fn vga_dac_palette(&self) -> &[[u8; 3]; 256] {
        &self.vga_dac_palette
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn render(&self) -> RenderResult {
        let bytes_per_pixel = 4;
        let width = CHAR_WIDTH * TEXT_MODE_COLS;
        let height = CHAR_HEIGHT * TEXT_MODE_ROWS;
        let mut data = vec![0; width * height * bytes_per_pixel];

        // Render all cells
        let mut i = 0;
        for row in 0..TEXT_MODE_ROWS {
            for col in 0..TEXT_MODE_COLS {
                let character = self.vram[i];
                i += 1;
                let text_attr = TextAttribute::from_byte(self.vram[i], self.blink_enabled);
                i += 1;
                render_text(
                    RenderTextArgs {
                        font: &self.font,
                        row,
                        col,
                        character,
                        text_attr,
                        vga_dac_palette: &self.vga_dac_palette,
                        stride: width,
                    },
                    &mut data,
                );
            }
        }

        RenderResult {
            data,
            width: width as u32,
            height: height as u32,
        }
    }
}
