use std::io::Stdout;

use anyhow::{Ok, Result};
use crossterm::{
    ExecutableCommand,
    cursor::{self, MoveTo},
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{self, Clear, ClearType},
};
use oxide86_core::computer::Computer;
use string_cmd::{StringEditor, events::event_to_command};

/// Implements command mode, returns true if we should quit
pub(crate) fn run_command_mode(computer: &mut Computer, stdout: &mut Stdout) -> Result<bool> {
    let mut editor = StringEditor::new();
    loop {
        stdout
            .execute(Clear(ClearType::All))?
            .execute(MoveTo(0, 0))?;

        println!();
        println!(" Oxide86 Command Mode\r");
        println!();
        println!(
            "   log exec - Toggle exec logging [{}]\r",
            if computer.exec_logging_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        );
        println!("   quit/q   - Quit Emulator\r");
        println!();
        print!("?> ");

        let event = event::read()?;
        if let Event::Key(key_event) = &event
            && key_event.kind == KeyEventKind::Press
        {
            match key_event.code {
                KeyCode::Esc => return Ok(false),
                KeyCode::Enter => {
                    let text = editor.get_text().trim().to_lowercase();
                    if text == "quit" || text == "q" {
                        return Ok(true);
                    }
                }
                _ => {}
            }
        }

        if let Some(command) = event_to_command(&event) {
            editor.execute(command);
        };

        stdout
            .execute(cursor::MoveToColumn(0))?
            .execute(terminal::Clear(terminal::ClearType::CurrentLine))?;
        log::info!("{}", editor.get_text());
        print!("?> {}\r", editor.get_text());
        stdout.execute(cursor::MoveToColumn(
            (editor.cursor_pos() + "?> ".len()) as u16,
        ))?;
    }
}
