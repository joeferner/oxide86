use crossterm::{
    ExecutableCommand, QueueableCommand, cursor, execute,
    style::{Color, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use oxide86_core::{
    TextModePalette,
    video::{
        CursorPosition, TEXT_MODE_COLS, TEXT_MODE_ROWS, VideoController, VideoMode,
        text::TextBuffer,
    },
};
use std::io::{self, Write};

// MIGRATED  /// Convert CP437 byte to Unicode character
// MIGRATED  /// CP437 is the original IBM PC character set
// MIGRATED  fn cp437_to_unicode(byte: u8) -> char {
// MIGRATED      // CP437 high characters (0x80-0xFF) to Unicode
// MIGRATED      const CP437_HIGH: [char; 128] = [
// MIGRATED          'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ',
// MIGRATED          'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú',
// MIGRATED          'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡',
// MIGRATED          '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟',
// MIGRATED          '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘',
// MIGRATED          '┌', '█', '▄', '▌', '▐', '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ',
// MIGRATED          '∞', 'φ', 'ε', '∩', '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²',
// MIGRATED          '■', ' ',
// MIGRATED      ];
// MIGRATED  
// MIGRATED      match byte {
// MIGRATED          0x00 => ' ',                 // NUL - display as space
// MIGRATED          0x20..=0x7E => byte as char, // Standard ASCII printable
// MIGRATED          0x7F => '⌂',                 // DEL - house symbol in CP437
// MIGRATED          0x80..=0xFF => CP437_HIGH[(byte - 0x80) as usize],
// MIGRATED          _ => byte as char, // Low control chars - pass through
// MIGRATED      }
// MIGRATED  }

/// Terminal-based video controller using crossterm
pub struct TerminalVideo {
    last_buffer: TextBuffer,
    last_cursor: Option<CursorPosition>,
    current_mode: VideoMode,
    /// VGA DAC palette (256 RGB triplets, 6-bit per component 0-63)
    vga_dac_palette: [[u8; 3]; 256],
}

impl TerminalVideo {
    pub fn new() -> Self {
// MIGRATED          let mut stdout = io::stdout();

// MIGRATED          // Enable raw mode and alternate screen
// MIGRATED          terminal::enable_raw_mode().unwrap();
// MIGRATED          execute!(
// MIGRATED              stdout,
// MIGRATED              EnterAlternateScreen,
// MIGRATED              SetForegroundColor(Color::White),
// MIGRATED              SetBackgroundColor(Color::Black),
// MIGRATED              terminal::Clear(ClearType::All),
// MIGRATED              cursor::Hide
// MIGRATED          )
// MIGRATED          .unwrap();

        Self {
            last_buffer: TextBuffer::new(),
            last_cursor: None,
            current_mode: VideoMode::Text {
                cols: TEXT_MODE_COLS,
                rows: TEXT_MODE_ROWS,
            },
            vga_dac_palette: Self::default_vga_dac_palette(),
        }
    }

    /// Create default VGA DAC palette (same as core::video::default_vga_palette)
    fn default_vga_dac_palette() -> [[u8; 3]; 256] {
        let mut palette = [[0u8; 3]; 256];
        // Initialize first 16 colors with EGA defaults (6-bit RGB values 0-63)
        for (i, entry) in palette.iter_mut().enumerate().take(16) {
            *entry = TextModePalette::get_dac_color(i as u8);
        }
        palette
    }

// MIGRATED      /// Convert 6-bit VGA DAC RGB (0-63) to 8-bit RGB (0-255)
// MIGRATED      /// Uses the standard VGA conversion: value * 255 / 63
// MIGRATED      fn dac_to_rgb(&self, dac_value: u8) -> u8 {
// MIGRATED          // Standard VGA DAC conversion: multiply by ~4.047619
// MIGRATED          // Using ((value << 2) | (value >> 4)) for accuracy
// MIGRATED          let val = dac_value & 0x3F; // Ensure 6-bit
// MIGRATED          (val << 2) | (val >> 4)
// MIGRATED      }

// MIGRATED      /// Get 8-bit RGB color from VGA DAC palette
// MIGRATED      fn get_palette_color(&self, color_index: u8) -> [u8; 3] {
// MIGRATED          let dac_color = self.vga_dac_palette[color_index as usize];
// MIGRATED          [
// MIGRATED              self.dac_to_rgb(dac_color[0]),
// MIGRATED              self.dac_to_rgb(dac_color[1]),
// MIGRATED              self.dac_to_rgb(dac_color[2]),
// MIGRATED          ]
// MIGRATED      }

// MIGRATED      /// Map VGA color to crossterm Color using the VGA DAC palette
// MIGRATED      fn vga_to_crossterm_color(&self, vga_color: u8) -> Color {
// MIGRATED          let color_tuple = self.get_palette_color(vga_color);
// MIGRATED          Color::Rgb {
// MIGRATED              r: color_tuple[0],
// MIGRATED              g: color_tuple[1],
// MIGRATED              b: color_tuple[2],
// MIGRATED          }
// MIGRATED      }
}

impl VideoController for TerminalVideo {
    fn update_display(&mut self, buffer: &TextBuffer) {
        let mut stdout = io::stdout();

        // Get actual mode dimensions
        let (actual_cols, actual_rows) = match self.current_mode {
            VideoMode::Text { cols, rows } => (cols, rows),
            _ => (TEXT_MODE_COLS, TEXT_MODE_ROWS),
        };

        // Only update changed cells for efficiency
        for row in 0..actual_rows {
            for col in 0..actual_cols {
                let idx = row * TEXT_MODE_COLS + col;
                if buffer[idx] != self.last_buffer[idx] {
                    let cell = &buffer[idx];

                    // Position cursor (crossterm uses 0-indexed coordinates)
                    stdout
                        .queue(cursor::MoveTo(col as u16, row as u16))
                        .unwrap();

                    // Set colors
                    stdout
                        .queue(SetForegroundColor(
                            self.vga_to_crossterm_color(cell.attribute.foreground),
                        ))
                        .unwrap();
                    stdout
                        .queue(SetBackgroundColor(
                            self.vga_to_crossterm_color(cell.attribute.background),
                        ))
                        .unwrap();

                    // Print character (convert CP437 to Unicode)
                    stdout
                        .queue(crossterm::style::Print(cp437_to_unicode(cell.character)))
                        .unwrap();
                }
            }
        }

        stdout.flush().unwrap();
        self.last_buffer.copy_from(buffer);
    }

    fn update_cursor(&mut self, position: CursorPosition) {
        // Only update if cursor position has changed
        if self.last_cursor == Some(position) {
            return;
        }

        let mut stdout = io::stdout();

        // Position cursor and show it
        stdout
            .queue(cursor::MoveTo(position.col as u16, position.row as u16))
            .unwrap();
        stdout.queue(cursor::Show).unwrap();
        stdout.flush().unwrap();

        self.last_cursor = Some(position);
    }

    fn set_video_mode(&mut self, mode: u8) {
        // Update current mode based on video mode number
        self.current_mode = match mode {
            0x00 | 0x01 => VideoMode::Text { cols: 40, rows: 25 },
            0x02 | 0x03 | 0x07 => VideoMode::Text { cols: 80, rows: 25 },
            0x04 | 0x05 => oxide86_core::video::VideoMode::Graphics320x200,
            0x06 => oxide86_core::video::VideoMode::Graphics640x200,
            _ => VideoMode::Text { cols: 80, rows: 25 }, // Default to text mode
        };

        let mut stdout = io::stdout();

        // Clear screen on mode change
        stdout
            .execute(terminal::Clear(terminal::ClearType::All))
            .unwrap();
        stdout.flush().unwrap();
    }

    fn force_redraw(&mut self, buffer: &TextBuffer) {
        // Reset cached buffer and cursor to force a full redraw
        self.last_buffer = TextBuffer::new();
        self.last_cursor = None;

        // Now update_display will redraw everything since all cells will differ
        self.update_display(buffer);
    }

    fn update_vga_dac_palette(&mut self, palette: &[[u8; 3]; 256]) {
        // Only force a full redraw if the palette actually changed
        if self.vga_dac_palette == *palette {
            return;
        }
        self.vga_dac_palette.copy_from_slice(palette);
        self.last_buffer = TextBuffer::new();
        self.last_cursor = None;
    }
}

// MIGRATED  impl Drop for TerminalVideo {
// MIGRATED      fn drop(&mut self) {
// MIGRATED          let mut stdout = io::stdout();
// MIGRATED  
// MIGRATED          // Restore terminal: leave alternate screen, disable raw mode, show cursor
// MIGRATED          stdout.execute(LeaveAlternateScreen).unwrap();
// MIGRATED          stdout.execute(cursor::Show).unwrap();
// MIGRATED          stdout.flush().unwrap();
// MIGRATED  
// MIGRATED          terminal::disable_raw_mode().unwrap();
// MIGRATED      }
// MIGRATED  }
