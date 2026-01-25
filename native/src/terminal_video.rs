use emu86_core::video::{
    CursorPosition, TEXT_MODE_COLS, TEXT_MODE_ROWS, TextAttribute, TextCell, VideoController,
};
use std::io::{self, Write};

/// Terminal-based video controller using ANSI escape codes
pub struct TerminalVideo {
    last_buffer: [TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS],
}

impl TerminalVideo {
    pub fn new() -> Self {
        // Clear screen and hide cursor
        print!("\x1b[2J\x1b[?25l");
        io::stdout().flush().unwrap();

        Self {
            last_buffer: [TextCell::default(); TEXT_MODE_COLS * TEXT_MODE_ROWS],
        }
    }

    fn attribute_to_ansi(attr: &TextAttribute) -> String {
        // Map VGA colors to ANSI color codes
        // ANSI: 30-37 (foreground), 40-47 (background)
        // VGA colors 0-7 map directly, 8-15 use bright variants
        let fg = if attr.foreground < 8 {
            30 + attr.foreground
        } else {
            // Bright colors (90-97)
            90 + (attr.foreground - 8)
        };

        let bg = if attr.background < 8 {
            40 + attr.background
        } else {
            // Bright backgrounds (100-107)
            100 + (attr.background - 8)
        };

        if attr.blink {
            format!("\x1b[{};{};5m", fg, bg)
        } else {
            format!("\x1b[{};{}m", fg, bg)
        }
    }
}

impl VideoController for TerminalVideo {
    fn update_display(&mut self, buffer: &[TextCell; TEXT_MODE_COLS * TEXT_MODE_ROWS]) {
        // Only update changed cells for efficiency
        for row in 0..TEXT_MODE_ROWS {
            for col in 0..TEXT_MODE_COLS {
                let idx = row * TEXT_MODE_COLS + col;
                if buffer[idx] != self.last_buffer[idx] {
                    // Position cursor (ANSI rows/cols are 1-indexed)
                    print!("\x1b[{};{}H", row + 1, col + 1);
                    // Set colors and print character
                    print!(
                        "{}{}",
                        Self::attribute_to_ansi(&buffer[idx].attribute),
                        buffer[idx].character as char
                    );
                }
            }
        }
        io::stdout().flush().unwrap();
        self.last_buffer.copy_from_slice(buffer);
    }

    fn update_cursor(&mut self, position: CursorPosition) {
        // Position cursor and show it (ANSI rows/cols are 1-indexed)
        print!("\x1b[{};{}H\x1b[?25h", position.row + 1, position.col + 1);
        io::stdout().flush().unwrap();
    }

    fn set_video_mode(&mut self, _mode: u8) {
        // Clear screen on mode change
        print!("\x1b[2J");
        io::stdout().flush().unwrap();
    }
}

impl Drop for TerminalVideo {
    fn drop(&mut self) {
        // Restore terminal on exit: reset colors, show cursor, move to bottom
        // Position cursor at row 26 (below the 25-row display)
        println!("\x1b[26;1H\x1b[0m\x1b[?25h");
        io::stdout().flush().unwrap();
    }
}
