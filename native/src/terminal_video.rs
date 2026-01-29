use crossterm::{
    ExecutableCommand, QueueableCommand, cursor, execute,
    style::{Color, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use emu86_core::video::{
    CursorPosition, TEXT_MODE_COLS, TEXT_MODE_ROWS, TextCell, VideoController,
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
    match vga_color {
        0 => Color::Black,
        1 => Color::Blue,
        2 => Color::Green,
        3 => Color::Cyan,
        4 => Color::Red,
        5 => Color::Magenta,
        6 => Color::Yellow,
        7 => Color::Grey,
        8 => Color::DarkGrey,
        9 => Color::DarkBlue,
        10 => Color::DarkGreen,
        11 => Color::DarkCyan,
        12 => Color::DarkRed,
        13 => Color::DarkMagenta,
        14 => Color::DarkYellow,
        15 => Color::White,
        _ => Color::White,
    }
}

/// Terminal-based video controller using crossterm
pub struct TerminalVideo {
    last_buffer: [TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS],
    last_cursor: Option<CursorPosition>,
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
        }
    }
}

impl VideoController for TerminalVideo {
    fn update_display(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {
        let mut stdout = io::stdout();

        // Only update changed cells for efficiency
        for row in 0..TEXT_MODE_ROWS {
            for col in 0..TEXT_MODE_COLS {
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

    fn set_video_mode(&mut self, _mode: u8) {
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
