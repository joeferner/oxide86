use std::{
    io::{Stdout, Write},
    sync::{Arc, RwLock},
};

use anyhow::Result;
use crossterm::{
    QueueableCommand, cursor,
    style::{Color, SetBackgroundColor, SetForegroundColor},
    terminal,
};
use oxide86_core::video::{
    VideoBuffer,
    renderer::dac_to_8bit,
    text::{TextAttribute, cp437_to_unicode},
    video_buffer::VideoMode,
};

#[derive(Debug, PartialEq)]
pub(crate) struct VideoCachedValue {
    character: u8,
    fg: Color,
    bg: Color,
}

pub(crate) fn draw_frame(
    video_cache: &mut Vec<VideoCachedValue>,
    buffer: &Arc<RwLock<VideoBuffer>>,
    stdout: &mut Stdout,
) -> Result<()> {
    let buffer = buffer.read().unwrap();
    if !buffer.is_dirty() {
        return Ok(());
    }

    // TODO use start address for scrolling

    if let VideoMode::Text { cols, rows } = buffer.mode() {
        let (term_cols, term_rows) = terminal::size()?;
        if (term_cols as usize) < cols || (term_rows as usize) < rows {
            video_cache.clear();
            stdout.queue(cursor::MoveTo(0, 0))?;
            stdout.queue(crossterm::style::ResetColor)?;
            stdout.queue(crossterm::style::Print(format!(
                "Terminal too small: need {}x{}, got {}x{}",
                cols, rows, term_cols, term_rows
            )))?;
            stdout.flush()?;
            return Ok(());
        }

        let mut addr = 0;
        let mut current_fg: Option<Color> = None;
        let mut current_bg: Option<Color> = None;
        let mut cursor_col: Option<usize> = None;
        let mut cursor_row: Option<usize> = None;
        for row in 0..rows {
            for col in 0..cols {
                let cache_location = addr;
                let cached_value = video_cache.get(cache_location);
                let character = buffer.read_vram(addr);
                addr += 1;
                let text_attr =
                    TextAttribute::from_byte(buffer.read_vram(addr), buffer.blink_enabled());
                addr += 1;
                let fg = vga_to_crossterm_color(text_attr.foreground, buffer.vga_dac_palette());
                let bg = vga_to_crossterm_color(text_attr.background, buffer.vga_dac_palette());
                let new_cached_value = VideoCachedValue { character, fg, bg };

                if cached_value.is_none() || cached_value.unwrap() != &new_cached_value {
                    // Only move cursor if it's not already at the right position
                    if cursor_col != Some(col) || cursor_row != Some(row) {
                        stdout.queue(cursor::MoveTo(col as u16, row as u16))?;
                    }

                    // Only set colors if they changed
                    if current_fg != Some(fg) {
                        stdout.queue(SetForegroundColor(fg))?;
                        current_fg = Some(fg);
                    }
                    if current_bg != Some(bg) {
                        stdout.queue(SetBackgroundColor(bg))?;
                        current_bg = Some(bg);
                    }

                    // Print character (convert CP437 to Unicode)
                    stdout.queue(crossterm::style::Print(cp437_to_unicode(character)))?;
                    cursor_col = Some(col + 1);
                    cursor_row = Some(row);

                    if video_cache.len() <= cache_location {
                        video_cache.resize_with(cache_location + 1, || VideoCachedValue {
                            character: 0,
                            fg: Color::Black,
                            bg: Color::Black,
                        });
                    }
                    video_cache[cache_location] = new_cached_value;
                } else {
                    // Cursor position is no longer tracked after skipping a cell
                    cursor_col = None;
                }
            }
        }

        let cursor_pos = buffer.get_cursor_position();
        stdout.queue(cursor::MoveTo(cursor_pos.col as u16, cursor_pos.row as u16))?;
    } else {
        todo!("graphics unsupported, clear the screen and write a message")
    }

    stdout.flush()?;

    Ok(())
}

/// Get 8-bit RGB color from VGA DAC palette
fn get_palette_color(color_index: u8, vga_dac_palette: &[[u8; 3]; 256]) -> [u8; 3] {
    let dac_color = vga_dac_palette[color_index as usize];
    [
        dac_to_8bit(dac_color[0]),
        dac_to_8bit(dac_color[1]),
        dac_to_8bit(dac_color[2]),
    ]
}

/// Map VGA color to crossterm Color using the VGA DAC palette
fn vga_to_crossterm_color(vga_color: u8, vga_dac_palette: &[[u8; 3]; 256]) -> Color {
    let color_tuple = get_palette_color(vga_color, vga_dac_palette);
    Color::Rgb {
        r: color_tuple[0],
        g: color_tuple[1],
        b: color_tuple[2],
    }
}
