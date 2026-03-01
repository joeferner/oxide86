use std::{
    io::Write,
    panic,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Result;
use clap::Parser;
use crossterm::{
    cursor,
    event::{self, DisableMouseCapture, Event, KeyEventKind},
    execute,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode},
};
use oxide86_core::video::VideoBuffer;
use oxide86_native_common::{cli::CommonCli, create_computer, logging::setup_logging};

use crate::{
    keyboard::{SCAN_CODE_F12, key_event_to_keypress},
    video::{VideoCachedValue, draw_frame},
};

mod keyboard;
mod video;

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
        terminal::Clear(ClearType::All)
    )?;

    // TODO
    let mut quit_from_command_mode = false;
    while computer.get_exit_code().is_none() && !quit_from_command_mode {
        if !computer.wait_for_key_press() {
            for _ in 0..BATCH_SIZE {
                computer.step();
                if computer.get_exit_code().is_some() || computer.wait_for_key_press() {
                    break;
                }
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
