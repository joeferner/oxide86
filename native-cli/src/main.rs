use std::{io::Write, panic, sync::Arc};

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

const BATCH_SIZE: usize = 1000;

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

    let cli = Cli::parse();
    let video_buffer = Arc::new(VideoBuffer::new());
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
    let quit_from_command_mode = false;
    while !computer.is_halted() {
        for _ in 0..BATCH_SIZE {
            computer.step();
            if computer.is_halted() {
                break;
            }
        }

        video_buffer.emu_try_flip();
        if let Some(frame) = video_buffer.ui_get_data() {
            let mut i = 0;
            for row in 0..TEXT_MODE_ROWS {
                for col in 0..TEXT_MODE_COLS {
                    let character = frame.vram[i];
                    i += 1;
                    let text_attr = TextAttribute::from_byte(frame.vram[i], frame.blink_enabled);
                    i += 1;

                    // Position cursor (crossterm uses 0-indexed coordinates)
                    stdout
                        .queue(cursor::MoveTo(col as u16, row as u16))
                        .unwrap();

                    // Set colors
                    stdout
                        .queue(SetForegroundColor(vga_to_crossterm_color(
                            text_attr.foreground,
                            &frame.vga_dac_palette,
                        )))
                        .unwrap();
                    stdout
                        .queue(SetBackgroundColor(vga_to_crossterm_color(
                            text_attr.background,
                            &frame.vga_dac_palette,
                        )))
                        .unwrap();

                    // Print character (convert CP437 to Unicode)
                    stdout
                        .queue(crossterm::style::Print(cp437_to_unicode(character)))
                        .unwrap();
                }
            }
            video_buffer.ui_mark_as_consumed();
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
