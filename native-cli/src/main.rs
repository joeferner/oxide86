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
    event::{self, DisableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode},
};
use oxide86_core::{
    scan_code::{
        SCAN_CODE_F12, SCAN_CODE_LEFT_ALT, SCAN_CODE_LEFT_CTRL, SCAN_CODE_LEFT_SHIFT,
        SCAN_CODE_RELEASE,
    },
    video::VideoBuffer,
};
use oxide86_native_common::{cli::CommonCli, create_computer, logging::setup_logging};

use crate::{
    command_mode::run_command_mode,
    keyboard::{char_requires_shift, key_code_to_scan_code},
    video::{VideoCachedValue, draw_frame},
};

mod command_mode;
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
    let mut last_modifiers = KeyModifiers::empty();

    // Enable raw mode and alternate screen
    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        EnterAlternateScreen,
        SetForegroundColor(Color::White),
        SetBackgroundColor(Color::Black),
        terminal::Clear(ClearType::All)
    )?;

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
                if key_event.kind == KeyEventKind::Press {
                    let scan_code = key_code_to_scan_code(&key_event.code);

                    // Check if it's F12 (command mode) - intercept for emulator, don't send to program
                    if scan_code == SCAN_CODE_F12 {
                        quit_from_command_mode = run_command_mode(&mut computer, &mut stdout)?;
                        if !quit_from_command_mode {
                            video_cache.clear();
                            draw_frame(&mut video_cache, &video_buffer, &mut stdout)?;
                        }
                    } else {
                        // Some terminals omit KeyModifiers::SHIFT for symbol characters
                        // (e.g. Shift+' produces '"' with no modifier reported), so also
                        // infer shift from the character itself.
                        let char_needs_shift = if let KeyCode::Char(c) = key_event.code {
                            char_requires_shift(c)
                        } else {
                            false
                        };
                        let shift_active =
                            key_event.modifiers.contains(KeyModifiers::SHIFT) || char_needs_shift;
                        let ctrl_active = key_event.modifiers.contains(KeyModifiers::CONTROL);
                        let alt_active = key_event.modifiers.contains(KeyModifiers::ALT);

                        let last_shift = last_modifiers.contains(KeyModifiers::SHIFT);
                        let last_ctrl = last_modifiers.contains(KeyModifiers::CONTROL);
                        let last_alt = last_modifiers.contains(KeyModifiers::ALT);

                        if !ctrl_active && last_ctrl {
                            computer.push_key_press(SCAN_CODE_RELEASE | SCAN_CODE_LEFT_CTRL);
                        }
                        if ctrl_active && !last_ctrl {
                            computer.push_key_press(SCAN_CODE_LEFT_CTRL);
                        }
                        if !alt_active && last_alt {
                            computer.push_key_press(SCAN_CODE_RELEASE | SCAN_CODE_LEFT_ALT);
                        }
                        if alt_active && !last_alt {
                            computer.push_key_press(SCAN_CODE_LEFT_ALT);
                        }
                        if !shift_active && last_shift {
                            computer.push_key_press(SCAN_CODE_RELEASE | SCAN_CODE_LEFT_SHIFT);
                        }
                        if shift_active && !last_shift {
                            computer.push_key_press(SCAN_CODE_LEFT_SHIFT);
                        }
                        computer.push_key_press(scan_code);
                        computer.push_key_press(SCAN_CODE_RELEASE | scan_code);
                        // Store effective modifiers so modifier-release is tracked correctly
                        // even when the terminal omitted SHIFT from the event.
                        last_modifiers = if shift_active {
                            key_event.modifiers | KeyModifiers::SHIFT
                        } else {
                            key_event.modifiers
                        };
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
