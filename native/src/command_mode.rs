use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::{Color, Print, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::ClearType;
use crossterm::{cursor, execute, terminal};
use emu86_core::{BackedDisk, Computer, DriveNumber};
use std::io::{self, Stdout, Write, stdout};

use crate::bios::NativeBios;
use crate::disk_backend::FileDiskBackend;

/// Read a line of input in raw terminal mode with basic editing support
fn read_line_raw() -> Option<String> {
    let mut line = String::new();
    let mut stdout = io::stdout();

    loop {
        // event::read() blocks until an event occurs
        if let Ok(Event::Key(key_event)) = event::read() {
            // Ignore key 'Release' events (mostly relevant for Windows)
            if key_event.kind == KeyEventKind::Release {
                continue;
            }

            match key_event.code {
                KeyCode::Enter => {
                    let _ = execute!(stdout, Print("\r\n"));
                    return Some(line);
                }
                KeyCode::Backspace => {
                    if !line.is_empty() {
                        line.pop();
                        // Move back, overwrite with space, move back again
                        let _ =
                            execute!(stdout, cursor::MoveLeft(1), Print(" "), cursor::MoveLeft(1));
                    }
                }
                KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    let _ = execute!(stdout, Print("^C\r\n"));
                    return None;
                }
                KeyCode::Char(c) => {
                    line.push(c);
                    let _ = execute!(stdout, Print(c));
                }
                _ => {}
            }
        }
    }
}

/// Command types for runtime operations
enum Command {
    Insert { drive: DriveNumber, path: String },
    Eject { drive: DriveNumber },
    ToggleLogging { enable: bool, log: Log },
    Resume,
    Quit,
    Help,
}

enum Log {
    Execution,
    Interrupt,
}

/// Parse a command from user input
fn parse_command(input: &str) -> Result<Command> {
    let input = input.trim();

    if input.is_empty() || input.eq_ignore_ascii_case("resume") {
        return Ok(Command::Resume);
    }

    if input.eq_ignore_ascii_case("q")
        || input.eq_ignore_ascii_case("quit")
        || input.eq_ignore_ascii_case("exit")
    {
        return Ok(Command::Quit);
    }

    if input.eq_ignore_ascii_case("help") {
        return Ok(Command::Help);
    }

    // Check for eject command: "eject a:" or "eject a" or "eject b:" or "eject b"
    if let Some(rest) = input.strip_prefix("eject ") {
        let drive_letter = rest.trim().trim_end_matches(':').to_uppercase();
        let drive = match drive_letter.as_str() {
            "A" => DriveNumber::floppy_a(),
            "B" => DriveNumber::floppy_b(),
            _ => return Err(anyhow::anyhow!("Invalid drive letter (use A or B)")),
        };
        return Ok(Command::Eject { drive });
    }

    // Check for load command: "load a: path" or "load a path" or "load b: path" or "load b path"
    if let Some(rest) = input.strip_prefix("load ") {
        let rest = rest.trim();

        // Parse drive letter (first character)
        if rest.is_empty() {
            return Err(anyhow::anyhow!("No drive letter specified"));
        }

        let drive_letter = rest.chars().next().unwrap().to_ascii_uppercase();

        // Skip drive letter and optional colon
        let after_drive = if rest.len() > 1 && rest.chars().nth(1) == Some(':') {
            &rest[2..] // Skip "a:"
        } else {
            &rest[1..] // Skip "a"
        };

        let path = after_drive.trim().to_string();

        if path.is_empty() {
            return Err(anyhow::anyhow!("No path specified after drive letter"));
        }

        let drive = match drive_letter {
            'A' => DriveNumber::floppy_a(),
            'B' => DriveNumber::floppy_b(),
            _ => return Err(anyhow::anyhow!("Invalid drive letter (use A or B)")),
        };

        return Ok(Command::Insert { drive, path });
    }

    // enable/disable logging
    if let Some(rest) = input.strip_prefix("log ") {
        let (enable, rest) = if let Some(rest) = rest.strip_prefix("enable ") {
            (true, rest)
        } else if let Some(rest) = rest.strip_prefix("disable ") {
            (false, rest)
        } else {
            return Err(anyhow::anyhow!("Invalid log command"));
        };

        let log = if rest == "exec" {
            Log::Execution
        } else if rest == "int" {
            Log::Interrupt
        } else {
            return Err(anyhow::anyhow!("Invalid log command"));
        };

        return Ok(Command::ToggleLogging { enable, log });
    }

    Err(anyhow::anyhow!("Invalid command. Type 'help' for usage."))
}

fn show_help(stdout: &mut Stdout) -> Result<()> {
    execute!(
        stdout,
        Print("Commands:\r\n"),
        Print("  load a path/to/disk.img   - Insert disk into drive A:\r\n"),
        Print("  load b path/to/disk.img   - Insert disk into drive B:\r\n"),
        Print("  eject a                   - Eject floppy from drive A:\r\n"),
        Print("  eject b                   - Eject floppy from drive B:\r\n"),
        Print("  log enable/disable exec   - Enable/Disable execution logging\r\n"),
        Print("  log enable/disable int    - Enable/Disable interrupt logging\r\n"),
        Print("  resume (or Enter)         - Resume emulation\r\n"),
        Print("  q, quit, exit             - Halt emulator and exit\r\n"),
    )?;
    Ok(())
}

/// Handle command mode for runtime operations (floppy swapping, etc.)
/// Returns true to continue emulation, false to halt
pub fn handle_command_mode<I, V>(
    computer: &mut Computer<NativeBios<Box<dyn emu86_core::DiskController>>, I, V>,
) -> Result<bool>
where
    I: emu86_core::IoDevice,
    V: emu86_core::VideoController,
{
    let mut stdout = stdout();

    // Save current screen state
    // We'll use an alternate screen buffer approach: clear screen, show menu,
    // then restore by redrawing the video buffer

    execute!(
        stdout,
        cursor::SavePosition,
        SetForegroundColor(Color::White),
        SetBackgroundColor(Color::Black),
        terminal::Clear(ClearType::All),
        cursor::MoveTo(0, 0),
    )?;

    execute!(stdout, Print("=== Command Mode (F12) ===\r\n"))?;
    show_help(&mut stdout)?;

    let should_continue = loop {
        execute!(stdout, Print("\r\nCommand> "))?;
        io::stdout().flush().ok();

        let input = match read_line_raw() {
            Some(line) => line,
            None => {
                execute!(stdout, Print("Cancelled.\r\n"))?;
                break true; // Continue emulation
            }
        };

        let command = match parse_command(&input) {
            Ok(cmd) => cmd,
            Err(e) => {
                execute!(stdout, Print(format!("Error: {}\r\n", e)))?;
                continue;
            }
        };

        match command {
            Command::Help => {
                show_help(&mut stdout)?;
            }
            Command::Resume => {
                execute!(stdout, Print("Resuming emulation...\r\n"))?;
                break true;
            }
            Command::Quit => {
                execute!(stdout, Print("Halting emulator...\r\n"))?;
                break false;
            }
            Command::Insert { drive, path } => match insert_floppy(computer, drive, &path) {
                Ok(()) => {
                    execute!(
                        stdout,
                        Print(format!(
                            "Successfully inserted {} into drive {}\r\n",
                            path,
                            format_drive(drive)
                        ))
                    )?;
                    log::info!("Inserted {} into drive 0x{:02X}", path, drive.to_standard());
                }
                Err(e) => {
                    execute!(stdout, Print(format!("Error: {}\r\n", e)))?;
                    log::error!("Failed to insert {}: {}", path, e);
                }
            },
            Command::Eject { drive } => match eject_floppy(computer, drive) {
                Ok(()) => {
                    execute!(
                        stdout,
                        Print(format!(
                            "Successfully ejected drive {}\r",
                            format_drive(drive)
                        ))
                    )?;
                    log::info!("Ejected drive 0x{:02X}", drive.to_standard());
                }
                Err(e) => {
                    execute!(stdout, Print(format!("Error: {}\r", e)))?;
                    log::error!("Failed to eject: {}", e);
                }
            },
            Command::ToggleLogging { enable, log } => match log {
                Log::Execution => computer.exec_logging_enabled = enable,
                Log::Interrupt => computer.set_log_interrupts(enable),
            },
        }
    };

    // Restore screen: clear, then force a full video update
    execute!(
        stdout,
        terminal::Clear(ClearType::All),
        cursor::RestorePosition,
    )?;

    // Force video controller to redraw the entire screen
    // Use force_video_redraw() instead of update_video() because the terminal
    // was just cleared and the video controller's cached state is out of sync
    computer.force_video_redraw();

    Ok(should_continue)
}

fn format_drive(drive: DriveNumber) -> String {
    match drive.to_standard() {
        0x00 => "A:".to_string(),
        0x01 => "B:".to_string(),
        n => format!("0x{:02X}", n),
    }
}

fn insert_floppy<I, V>(
    computer: &mut Computer<NativeBios<Box<dyn emu86_core::DiskController>>, I, V>,
    drive: DriveNumber,
    path: &str,
) -> Result<()>
where
    I: emu86_core::IoDevice,
    V: emu86_core::VideoController,
{
    // Verify drive is a floppy
    if drive.to_standard() != 0x00 && drive.to_standard() != 0x01 {
        return Err(anyhow::anyhow!("Can only swap floppy drives (A: or B:)"));
    }

    // Load the disk image
    let backend = FileDiskBackend::open(path, false)?;
    let disk = BackedDisk::new(backend)?;

    // Get mutable access to BIOS and insert the floppy
    let bios = computer.bios_mut();
    bios.insert_floppy(drive, Box::new(disk))
        .map_err(|e| anyhow::anyhow!("Failed to insert floppy: {}", e))?;

    Ok(())
}

fn eject_floppy<I, V>(
    computer: &mut Computer<NativeBios<Box<dyn emu86_core::DiskController>>, I, V>,
    drive: DriveNumber,
) -> Result<()>
where
    I: emu86_core::IoDevice,
    V: emu86_core::VideoController,
{
    // Verify drive is a floppy
    if drive.to_standard() != 0x00 && drive.to_standard() != 0x01 {
        return Err(anyhow::anyhow!("Can only eject floppy drives (A: or B:)"));
    }

    // Get mutable access to BIOS and eject the floppy
    let bios = computer.bios_mut();

    match bios.eject_floppy(drive) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(anyhow::anyhow!("No disk in drive {}", format_drive(drive))),
        Err(e) => Err(anyhow::anyhow!("Failed to eject: {}", e)),
    }
}
