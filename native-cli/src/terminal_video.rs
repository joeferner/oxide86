use crossterm::{
    ExecutableCommand, QueueableCommand, cursor, execute,
    style::{Color, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use emu86_core::{
    TextModePalette,
    video::{CursorPosition, TEXT_MODE_COLS, TEXT_MODE_ROWS, TextCell, VideoController, VideoMode},
};
use std::io::{self, Write};

/// Convert CP437 byte to Unicode character
/// CP437 is the original IBM PC character set
fn cp437_to_unicode(byte: u8) -> char {
    // CP437 high characters (0x80-0xFF) to Unicode
    const CP437_HIGH: [char; 128] = [
        'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ',
        'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú',
        'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡',
        '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟',
        '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘',
        '┌', '█', '▄', '▌', '▐', '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ',
        '∞', 'φ', 'ε', '∩', '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²',
        '■', ' ',
    ];

    match byte {
        0x00 => ' ',                 // NUL - display as space
        0x20..=0x7E => byte as char, // Standard ASCII printable
        0x7F => '⌂',                 // DEL - house symbol in CP437
        0x80..=0xFF => CP437_HIGH[(byte - 0x80) as usize],
        _ => byte as char, // Low control chars - pass through
    }
}

/// Map VGA color to crossterm Color
fn vga_to_crossterm_color(vga_color: u8) -> Color {
    let color_tuple = TextModePalette::get_color(vga_color);
    Color::Rgb {
        r: color_tuple[0],
        g: color_tuple[1],
        b: color_tuple[2],
    }
}

/// Terminal-based video controller using crossterm
pub struct TerminalVideo {
    last_buffer: [TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS],
    last_cursor: Option<CursorPosition>,
    current_mode: VideoMode,
}

impl TerminalVideo {
    pub fn new() -> Self {
        let mut stdout = io::stdout();

        // Enable raw mode and alternate screen
        terminal::enable_raw_mode().unwrap();
        execute!(
            stdout,
            EnterAlternateScreen,
            SetForegroundColor(Color::White),
            SetBackgroundColor(Color::Black),
            terminal::Clear(ClearType::All),
            cursor::Hide
        )
        .unwrap();

        Self {
            last_buffer: [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS],
            last_cursor: None,
            current_mode: VideoMode::Text {
                cols: TEXT_MODE_COLS,
                rows: TEXT_MODE_ROWS,
            },
        }
    }
}

impl VideoController for TerminalVideo {
    fn update_display(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {
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
                        .queue(SetForegroundColor(vga_to_crossterm_color(
                            cell.attribute.foreground,
                        )))
                        .unwrap();
                    stdout
                        .queue(SetBackgroundColor(vga_to_crossterm_color(
                            cell.attribute.background,
                        )))
                        .unwrap();

                    // Print character (convert CP437 to Unicode)
                    stdout
                        .queue(crossterm::style::Print(cp437_to_unicode(cell.character)))
                        .unwrap();
                }
            }
        }

        stdout.flush().unwrap();
        self.last_buffer.copy_from_slice(buffer);
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
            0x04 | 0x05 => emu86_core::video::VideoMode::Graphics320x200,
            0x06 => emu86_core::video::VideoMode::Graphics640x200,
            _ => VideoMode::Text { cols: 80, rows: 25 }, // Default to text mode
        };

        let mut stdout = io::stdout();

        // Clear screen on mode change
        stdout
            .execute(terminal::Clear(terminal::ClearType::All))
            .unwrap();
        stdout.flush().unwrap();
    }

    fn force_redraw(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {
        // Reset cached buffer and cursor to force a full redraw
        self.last_buffer = [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS];
        self.last_cursor = None;

        // Now update_display will redraw everything since all cells will differ
        self.update_display(buffer);
    }
}

impl Drop for TerminalVideo {
    fn drop(&mut self) {
        let mut stdout = io::stdout();

        // Restore terminal: leave alternate screen, disable raw mode, show cursor
        stdout.execute(LeaveAlternateScreen).unwrap();
        stdout.execute(cursor::Show).unwrap();
        stdout.flush().unwrap();

        terminal::disable_raw_mode().unwrap();
    }
}
