use std::{
    io::{Stdout, Write},
    sync::{Arc, RwLock},
};

use anyhow::Result;
use crossterm::{
    QueueableCommand, cursor,
    style::{Color, SetBackgroundColor, SetForegroundColor},
};
use oxide86_core::video::{
    TEXT_MODE_COLS, TEXT_MODE_ROWS, VideoBuffer,
    renderer::dac_to_8bit,
    text::{TextAttribute, cp437_to_unicode},
};

#[derive(Debug, PartialEq)]
pub struct VideoCachedValue {
    character: u8,
    fg: Color,
    bg: Color,
}

pub fn draw_frame(
    video_cache: &mut Vec<VideoCachedValue>,
    buffer: &Arc<RwLock<VideoBuffer>>,
    stdout: &mut Stdout,
) -> Result<()> {
    let buffer = buffer.read().unwrap();
    if !buffer.is_dirty() {
        return Ok(());
    }

    let mut addr = 0;
    let mut changed = false;
    for row in 0..TEXT_MODE_ROWS {
        for col in 0..TEXT_MODE_COLS {
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
                // Position cursor (crossterm uses 0-indexed coordinates)
                stdout.queue(cursor::MoveTo(col as u16, row as u16))?;

                // Set colors
                stdout.queue(SetForegroundColor(fg))?;
                stdout.queue(SetBackgroundColor(bg))?;

                // Print character (convert CP437 to Unicode)
                stdout.queue(crossterm::style::Print(cp437_to_unicode(character)))?;

                changed = true;

                if video_cache.len() <= cache_location {
                    video_cache.resize_with(cache_location + 1, || VideoCachedValue {
                        character: 0,
                        fg: Color::Black,
                        bg: Color::Black,
                    });
                }
                video_cache[cache_location] = new_cached_value;
            }
        }
    }
    if changed {
        stdout.flush()?;
    }

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
