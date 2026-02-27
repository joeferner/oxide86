use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

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

pub struct VideoData {
    /// Raw video RAM (64KB).
    /// In CGA/text modes: framebuffer at B8000-BFFFF.
    /// In EGA mode 0x0D: 4 planes × EGA_PLANE_SIZE bytes (plane N at vram[N*EGA_PLANE_SIZE..]).
    /// In VGA mode 0x13: linear framebuffer vram[0..64000], 1 byte per pixel.
    /// Persists across mode changes, just like real hardware.
    pub vram: Vec<u8>,
    pub font: Cp437Font,
    /// VGA DAC palette registers (256 entries, each with 6-bit RGB components)
    pub vga_dac_palette: [[u8; 3]; 256],
    /// Blink/intensity mode for text attribute bit 7.
    /// true  = bit 7 enables character blinking (8 background colors, default)
    /// false = bit 7 selects high-intensity background (16 background colors, no blink)
    pub blink_enabled: bool,
    pub cursor_loc: u16,
}

impl VideoData {
    pub fn new() -> Self {
        let mut vram = vec![0; VIDEO_MEMORY_SIZE];
        for i in (0..TEXT_MODE_SIZE).step_by(2) {
            vram[i] = 0x20; // space
            vram[i + 1] = 0x07; // Light Gray on Black
        }
        Self {
            vram,
            font: Cp437Font::new(),
            vga_dac_palette: Self::default_vga_dac_palette(),
            blink_enabled: false,
            cursor_loc: 0,
        }
    }

    fn copy_from(&mut self, src: &VideoData) {
        self.vram.as_mut_slice().copy_from_slice(&src.vram);
        self.font = src.font.clone();
        self.vga_dac_palette
            .as_mut_slice()
            .copy_from_slice(&src.vga_dac_palette);
        self.blink_enabled = src.blink_enabled;
        self.cursor_loc = src.cursor_loc;
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

pub struct VideoBuffer {
    front: AtomicPtr<VideoData>, // UI reads from here
    back: AtomicPtr<VideoData>,  // Emulator writes here

    // Flags for synchronization
    pub has_new_data: AtomicBool,
    pub ui_consumed: AtomicBool,
}

impl VideoBuffer {
    pub fn new() -> Self {
        let b1 = Box::into_raw(Box::new(VideoData::new()));
        let b2 = Box::into_raw(Box::new(VideoData::new()));

        Self {
            front: AtomicPtr::new(b1),
            back: AtomicPtr::new(b2),
            has_new_data: AtomicBool::new(false),
            ui_consumed: AtomicBool::new(true), // Start ready to accept
        }
    }

    /// UI THREAD: Called during the requestAnimationFrame loop
    pub fn ui_get_data(&self) -> Option<&VideoData> {
        // Only provide data if the emulator says there's something new
        if self.has_new_data.load(Ordering::Acquire) {
            let ptr = self.front.load(Ordering::Acquire);
            return Some(unsafe { &*ptr });
        }
        None
    }

    /// UI THREAD: Call this after pixels.render() is done
    pub fn ui_mark_as_consumed(&self) {
        self.has_new_data.store(false, Ordering::Release);
        self.ui_consumed.store(true, Ordering::Release);
    }

    /// EMULATOR THREAD: Get the buffer to write to
    #[allow(clippy::mut_from_ref)]
    pub fn emu_get_back_buffer_mut(&self) -> &mut VideoData {
        let ptr = self.back.load(Ordering::Acquire);
        unsafe { &mut *ptr }
    }

    /// EMULATOR THREAD: Get the buffer to read from
    pub fn emu_get_back_buffer(&self) -> &VideoData {
        let ptr = self.back.load(Ordering::Acquire);
        unsafe { &*ptr }
    }

    /// EMULATOR THREAD: The "Internal Flip"
    /// Call this whenever the emulator reaches a point where it wants
    /// the UI to see the current state.
    pub fn emu_try_flip(&self) {
        // Only flip if the UI has finished reading the previous front buffer
        if self.ui_consumed.load(Ordering::Acquire) {
            let back_ptr = self.back.load(Ordering::Relaxed);
            let front_ptr = self.front.load(Ordering::Relaxed);

            // Swap the pointers
            self.back.store(front_ptr, Ordering::Release);
            self.front.store(back_ptr, Ordering::Release);

            // PERSISTENCE: Copy current state to the new back buffer
            // This is safe because the UI is NOT reading 'front' yet
            // (has_new_data is still false) and the emulator hasn't
            // resumed work yet.
            unsafe {
                (*front_ptr).copy_from(&*back_ptr);
            }

            // Signal to UI that 'front' is ready
            self.ui_consumed.store(false, Ordering::Release);
            self.has_new_data.store(true, Ordering::Release);
        }
    }
}
