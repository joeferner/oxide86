use std::io::Stdout;

use anyhow::Result;
use crossterm::{
    ExecutableCommand,
    cursor::{self, MoveTo},
    event::{self, Event, KeyCode, KeyEventKind},
    style::{Color, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use oxide86_core::{
    computer::Computer,
    disk::{BackedDisk, DriveNumber},
};
use oxide86_native_common::{disk::FileDiskBackend, parse_disk_spec};
use string_cmd::{StringEditor, events::event_to_command};

/// Implements command mode, returns true if we should quit
pub(crate) fn run_command_mode(computer: &mut Computer, stdout: &mut Stdout) -> Result<bool> {
    let mut editor = StringEditor::new();
    let mut message: Option<String> = None;
    loop {
        stdout
            .execute(SetForegroundColor(Color::White))?
            .execute(SetBackgroundColor(Color::Black))?
            .execute(Clear(ClearType::All))?
            .execute(MoveTo(0, 0))?;

        println!();
        println!(" Oxide86 Command Mode\r");
        println!();
        println!(" Commands:\r");
        println!("   load a path/to/disk.img   - Insert disk into drive A:\r");
        println!("   load b path/to/disk.img   - Insert disk into drive B:\r");
        println!("   eject a                   - Eject floppy from drive A:\r");
        println!("   eject b                   - Eject floppy from drive B:\r");
        println!(
            "   log exec/l                - Toggle exec logging [{}]\r",
            if computer.exec_logging_enabled() {
                "enabled"
            } else {
                "disabled"
            }
        );
        println!("   resume/enter              - Resume execution\r");
        println!("   quit/q                    - Quit Emulator\r");
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
                        Command::EjectA => {
                            computer.set_floppy_disk(DriveNumber::floppy_a(), None);
                            editor = StringEditor::new();
                            message = Some("Disk A: ejected".to_string());
                        }
                        Command::EjectB => {
                            computer.set_floppy_disk(DriveNumber::floppy_b(), None);
                            editor = StringEditor::new();
                            message = Some("Disk B: ejected".to_string());
                        }
                        Command::LoadA(filename) => {
                            let (path, read_only) = parse_disk_spec(&filename);
                            match FileDiskBackend::open(path, read_only) {
                                Ok(backend) => match BackedDisk::new(backend) {
                                    Ok(disk) => {
                                        computer.set_floppy_disk(
                                            DriveNumber::floppy_a(),
                                            Some(Box::new(disk)),
                                        );
                                        editor = StringEditor::new();
                                        message = Some("Disk A: loaded".to_string());
                                    }
                                    Err(err) => {
                                        message = Some(format!("Failed to load disk A: {err}"));
                                    }
                                },
                                Err(err) => {
                                    message = Some(format!("Failed to load disk A: {err}"));
                                }
                            }
                        }
                        Command::LoadB(filename) => {
                            let (path, read_only) = parse_disk_spec(&filename);
                            match FileDiskBackend::open(path, read_only) {
                                Ok(backend) => match BackedDisk::new(backend) {
                                    Ok(disk) => {
                                        computer.set_floppy_disk(
                                            DriveNumber::floppy_b(),
                                            Some(Box::new(disk)),
                                        );
                                        editor = StringEditor::new();
                                        message = Some("Disk B: loaded".to_string());
                                    }
                                    Err(err) => {
                                        message = Some(format!("Failed to load disk B: {err}"));
                                    }
                                },
                                Err(err) => {
                                    message = Some(format!("Failed to load disk B: {err}"));
                                }
                            }
                        }
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
    EjectA,
    EjectB,
    LoadA(String),
    LoadB(String),
    Invalid,
}

impl Command {
    pub(crate) fn parse(text: &str) -> Self {
        let text = text.trim();
        if text == "quit" || text == "q" {
            Self::Quit
        } else if text == "resume" || text.is_empty() {
            Self::Resume
        } else if text == "log exec" || text == "l" {
            Self::ToggleLogExec
        } else if text == "eject a" {
            Self::EjectA
        } else if text == "eject b" {
            Self::EjectB
        } else if let Some(filename) = text.strip_prefix("load a ") {
            Self::LoadA(filename.trim().to_string())
        } else if let Some(filename) = text.strip_prefix("load b ") {
            Self::LoadB(filename.trim().to_string())
        } else {
            Self::Invalid
        }
    }
}
