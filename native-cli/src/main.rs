use std::panic;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, Event, KeyEventKind},
    execute,
    style::Print,
    terminal::{LeaveAlternateScreen, disable_raw_mode},
};
use oxide86_native_common::logging::setup_logging;

fn main() -> Result<()> {
    setup_logging()?;

    let default_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        default_panic(info);
    }));

    // TODO
    let quit_from_command_mode = false;

    // Disable mouse capture before exiting
    execute!(std::io::stdout(), DisableMouseCapture).context("Failed to disable mouse capture")?;

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

    Ok(())
}
