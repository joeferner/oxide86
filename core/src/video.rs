// Video memory constants
pub const VIDEO_MEMORY_START: usize = 0xB8000;
pub const VIDEO_MEMORY_END: usize = 0xBFFFF;
pub const VIDEO_MEMORY_SIZE: usize = VIDEO_MEMORY_END - VIDEO_MEMORY_START + 1; // 32KB

// Text mode dimensions
pub const TEXT_MODE_COLS: usize = 80;
pub const TEXT_MODE_ROWS: usize = 25;
pub const TEXT_MODE_BUFFER_SIZE: usize = TEXT_MODE_COLS * TEXT_MODE_ROWS * 2; // char + attr

// VGA color constants
#[allow(dead_code)]
pub mod colors {
    pub const BLACK: u8 = 0x0;
    pub const BLUE: u8 = 0x1;
    pub const GREEN: u8 = 0x2;
    pub const CYAN: u8 = 0x3;
    pub const RED: u8 = 0x4;
    pub const MAGENTA: u8 = 0x5;
    pub const BROWN: u8 = 0x6;
    pub const LIGHT_GRAY: u8 = 0x7;
    pub const DARK_GRAY: u8 = 0x8;
    pub const LIGHT_BLUE: u8 = 0x9;
    pub const LIGHT_GREEN: u8 = 0xA;
    pub const LIGHT_CYAN: u8 = 0xB;
    pub const LIGHT_RED: u8 = 0xC;
    pub const LIGHT_MAGENTA: u8 = 0xD;
    pub const YELLOW: u8 = 0xE;
    pub const WHITE: u8 = 0xF;
}

/// VGA text mode character attribute
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextAttribute {
    pub foreground: u8, // 4 bits
    pub background: u8, // 3 bits
    pub blink: bool,    // 1 bit
}

impl TextAttribute {
    /// Create from VGA attribute byte
    pub fn from_byte(byte: u8) -> Self {
        Self {
            foreground: byte & 0x0F,
            background: (byte >> 4) & 0x07,
            blink: (byte & 0x80) != 0,
        }
    }

    /// Convert to VGA attribute byte
    pub fn to_byte(&self) -> u8 {
        let mut byte = self.foreground & 0x0F;
        byte |= (self.background & 0x07) << 4;
        if self.blink {
            byte |= 0x80;
        }
        byte
    }
}

impl Default for TextAttribute {
    fn default() -> Self {
        Self {
            foreground: colors::LIGHT_GRAY,
            background: colors::BLACK,
            blink: false,
        }
    }
}

/// A single character cell in text mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCell {
    pub character: u8,
    pub attribute: TextAttribute,
}

impl Default for TextCell {
    fn default() -> Self {
        Self {
            character: 0x20, // Space character
            attribute: TextAttribute::default(),
        }
    }
}

/// Cursor position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorPosition {
    pub row: usize,
    pub col: usize,
}

/// Video controller trait - platform-specific implementations provide rendering
pub trait VideoController {
    /// Called when video memory is updated
    /// Provides the entire text buffer for rendering
    fn update_display(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]);

    /// Update cursor position
    fn update_cursor(&mut self, position: CursorPosition);

    /// Called on mode changes (future: support multiple video modes)
    fn set_video_mode(&mut self, mode: u8);
}

/// Null video controller (no display)
#[derive(Debug, Default)]
pub struct NullVideoController;

impl VideoController for NullVideoController {
    fn update_display(&mut self, _buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {}
    fn update_cursor(&mut self, _position: CursorPosition) {}
    fn set_video_mode(&mut self, _mode: u8) {}
}

/// Core video state management
pub struct Video {
    /// Current cursor position
    cursor: CursorPosition,
    /// Text mode buffer (parsed representation)
    buffer: [TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS],
    /// Current video mode
    mode: u8,
    /// Active display page (0-7 for text modes)
    active_page: u8,
    /// Dirty flag to minimize unnecessary updates
    dirty: bool,
}

impl Video {
    pub fn new() -> Self {
        Self {
            cursor: CursorPosition::default(),
            buffer: [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS],
            mode: 0x03, // 80x25 text mode
            active_page: 0,
            dirty: false,
        }
    }

    /// Read a single byte from video memory
    pub fn read_byte(&self, offset: usize) -> u8 {
        if offset >= TEXT_MODE_BUFFER_SIZE {
            return 0;
        }

        let cell_index = offset / 2;
        if cell_index >= self.buffer.len() {
            return 0;
        }

        if offset.is_multiple_of(2) {
            // Even offset: character
            self.buffer[cell_index].character
        } else {
            // Odd offset: attribute
            self.buffer[cell_index].attribute.to_byte()
        }
    }

    /// Update a single byte in video memory
    pub fn write_byte(&mut self, offset: usize, value: u8) {
        if offset >= TEXT_MODE_BUFFER_SIZE {
            return; // Out of text mode range
        }

        let cell_index = offset / 2;
        if cell_index >= self.buffer.len() {
            return;
        }

        if offset.is_multiple_of(2) {
            // Even offset: character
            self.buffer[cell_index].character = value;
        } else {
            // Odd offset: attribute
            self.buffer[cell_index].attribute = TextAttribute::from_byte(value);
        }
        self.dirty = true;
    }

    /// Update a word (char + attr) in video memory
    pub fn write_word(&mut self, offset: usize, value: u16) {
        self.write_byte(offset, (value & 0xFF) as u8);
        self.write_byte(offset + 1, (value >> 8) as u8);
    }

    /// Get the current buffer for rendering
    pub fn get_buffer(&self) -> &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS] {
        &self.buffer
    }

    /// Check if display needs updating
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark as clean after rendering
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Set cursor position
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        if row < TEXT_MODE_ROWS && col < TEXT_MODE_COLS {
            self.cursor = CursorPosition { row, col };
        }
    }

    /// Get cursor position
    pub fn get_cursor(&self) -> CursorPosition {
        self.cursor
    }

    /// Set video mode
    pub fn set_mode(&mut self, mode: u8) {
        self.mode = mode;
        self.dirty = true;
    }

    /// Get current video mode
    pub fn get_mode(&self) -> u8 {
        self.mode
    }

    /// Set active display page
    pub fn set_active_page(&mut self, page: u8) {
        self.active_page = page;
        self.dirty = true;
    }

    /// Get active display page
    pub fn get_active_page(&self) -> u8 {
        self.active_page
    }
}

impl Default for Video {
    fn default() -> Self {
        Self::new()
    }
}

/// VGA I/O port handler for cursor control
pub struct VgaIoPorts {
    crtc_index: u8,
    cursor_location_high: u8,
    cursor_location_low: u8,
}

impl VgaIoPorts {
    pub fn new() -> Self {
        Self {
            crtc_index: 0,
            cursor_location_high: 0,
            cursor_location_low: 0,
        }
    }

    /// Handle write to CRT controller index register (0x3D4)
    pub fn write_index(&mut self, value: u8) {
        self.crtc_index = value;
    }

    /// Handle write to CRT controller data register (0x3D5)
    /// Returns Some(CursorPosition) if cursor position was updated
    pub fn write_data(&mut self, value: u8) -> Option<CursorPosition> {
        match self.crtc_index {
            0x0E => {
                // Cursor location high byte
                self.cursor_location_high = value;
                None
            }
            0x0F => {
                // Cursor location low byte
                self.cursor_location_low = value;
                // Calculate cursor position
                let offset =
                    ((self.cursor_location_high as u16) << 8) | (self.cursor_location_low as u16);
                let row = (offset as usize) / TEXT_MODE_COLS;
                let col = (offset as usize) % TEXT_MODE_COLS;
                Some(CursorPosition { row, col })
            }
            _ => None,
        }
    }

    /// Handle read from data register
    pub fn read_data(&self) -> u8 {
        match self.crtc_index {
            0x0E => self.cursor_location_high,
            0x0F => self.cursor_location_low,
            _ => 0xFF,
        }
    }
}

impl Default for VgaIoPorts {
    fn default() -> Self {
        Self::new()
    }
}
