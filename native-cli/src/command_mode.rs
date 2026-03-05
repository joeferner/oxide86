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
    let mut message: Option<String> = None;
    loop {
        stdout
            .execute(Clear(ClearType::All))?
            .execute(MoveTo(0, 0))?;

        println!();
        println!(" Oxide86 Command Mode\r");
        println!();
        println!(
            "   log exec/l - Toggle exec logging [{}]\r",
            if computer.exec_logging_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        );
        println!("   resume/esc - Resume execution\r");
        println!("   quit/q     - Quit Emulator\r");
        println!();
        if let Some(error) = &message {
            println!("{error}");
        } else {
            println!();
        }
        stdout
            .execute(cursor::MoveToColumn(0))?
            .execute(terminal::Clear(terminal::ClearType::CurrentLine))?;
        print!("?> {}\r", editor.get_text());
        stdout.execute(cursor::MoveToColumn(
            (editor.cursor_pos() + "?> ".len()) as u16,
        ))?;

        let event = event::read()?;
        if let Event::Key(key_event) = &event
            && key_event.kind == KeyEventKind::Press
        {
            match key_event.code {
                KeyCode::Esc => return Ok(false),
                KeyCode::Enter => {
                    let text = editor.get_text();
                    let cmd = Command::parse(text);
                    match cmd {
                        Command::ToggleLogExec => {
                            computer.set_exec_logging_enabled(!computer.exec_logging_enabled());
                            editor = StringEditor::new();
                            message = Some(format!(
                                "Execution logging {}",
                                if computer.exec_logging_enabled() {
                                    "enabled"
                                } else {
                                    "disabled"
                                }
                            ));
                        }
                        Command::Quit => return Ok(true),
                        Command::Resume => return Ok(false),
                        Command::Invalid => message = Some(format!("Invalid command: {text}")),
                    }
                }
                _ => {}
            }
        }

        if let Some(command) = event_to_command(&event) {
            editor.execute(command);
        };
    }
}

enum Command {
    ToggleLogExec,
    Quit,
    Resume,
    Invalid,
}

impl Command {
    pub fn parse(text: &str) -> Self {
        let text = text.trim().to_lowercase();
        if text == "quit" || text == "q" {
            Self::Quit
        } else if text == "resume" || text == "" {
            Self::Resume
        } else if text == "log exec" || text == "l" {
            Self::ToggleLogExec
        } else {
            Self::Invalid
        }
    }
}
