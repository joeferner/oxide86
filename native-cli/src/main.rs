use std::{
    io::{Stdout, Write},
    panic,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Result;
use clap::Parser;
use crossterm::{
    QueueableCommand, cursor,
    event::{self, DisableMouseCapture, Event, KeyEventKind},
    execute,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode},
};
use oxide86_core::video::{
    TEXT_MODE_COLS, TEXT_MODE_ROWS, VideoBuffer,
    renderer::dac_to_8bit,
    text::{TextAttribute, cp437_to_unicode},
};
use oxide86_native_common::{cli::CommonCli, create_computer, logging::setup_logging};

use crate::keyboard::{SCAN_CODE_F12, key_event_to_keypress};

mod keyboard;

const BATCH_SIZE: usize = 1000;

#[derive(Debug, PartialEq)]
struct VideoCachedValue {
    character: u8,
    fg: Color,
    bg: Color,
}

#[derive(Parser)]
#[command(name = "Oxide86")]
#[command(about = "Intel 8086 CPU Emulator", long_about = None)]
#[command(
    after_help = "During emulation:\n  Press F12 to enter command mode for floppy swapping and other runtime operations."
)]
struct Cli {
    #[command(flatten)]
    common: CommonCli,
}

fn main() -> Result<()> {
    setup_logging()?;

    let default_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        default_panic(info);
    }));

    let mut video_cache: Vec<VideoCachedValue> = vec![];
    let cli = Cli::parse();
    let video_buffer = Arc::new(RwLock::new(VideoBuffer::new()));
    let mut computer = create_computer(&cli.common, video_buffer.clone())?;

    let mut stdout = std::io::stdout();

    // Enable raw mode and alternate screen
    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        EnterAlternateScreen,
        SetForegroundColor(Color::White),
        SetBackgroundColor(Color::Black),
        terminal::Clear(ClearType::All),
        cursor::Hide
    )?;

    // TODO
    let mut quit_from_command_mode = false;
    while computer.get_exit_code().is_none() && !quit_from_command_mode {
        for _ in 0..BATCH_SIZE {
            computer.step();
            if computer.get_exit_code().is_some() {
                break;
            }
        }

        draw_frame(&mut video_cache, &video_buffer, &mut stdout)?;

        while event::poll(Duration::from_secs(0))? {
            if let Event::Key(key_event) = event::read()? {
                // Only process key press events; ignore release and repeat to avoid
                // duplicate input on Windows (which emits both Press and Release events)
                if key_event.kind != KeyEventKind::Press {
                    // skip
                } else {
                    let key = key_event_to_keypress(&key_event);

                    // Check if it's F12 (command mode) - intercept for emulator, don't send to program
                    if key.scan_code == SCAN_CODE_F12 {
                        quit_from_command_mode = true;
                    } else {
                        // Fire INT 09h (keyboard hardware interrupt) for all other keys
                        computer.push_keyboard_key(key);
                    }
                }
            }
        }
    }

    // If computer halted naturally (not from command mode quit), wait for keypress
    if !quit_from_command_mode {
        execute!(std::io::stdout(), Print("\nPress any key to exit..."))?;
        loop {
            if let Ok(Event::Key(key_event)) = event::read() {
                // Exit on any key press
                if let KeyEventKind::Press = key_event.kind {
                    break;
                }
            }
        }
    }

    // Disable mouse capture before exiting
    execute!(
        stdout,
        DisableMouseCapture,
        LeaveAlternateScreen,
        cursor::Show
    )?;
    stdout.flush()?;

    terminal::disable_raw_mode()?;

    Ok(())
}

fn draw_frame(
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
