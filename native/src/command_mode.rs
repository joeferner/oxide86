use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use emu86_core::{BackedDisk, Computer, DriveNumber};
use std::io::{self, Write};

use crate::bios::NativeBios;
use crate::disk_backend::FileDiskBackend;

/// Read a line of input in raw terminal mode with basic editing support
fn read_line_raw() -> Option<String> {
    let mut line = String::new();
    let mut stdout = io::stdout();

    loop {
        if let Ok(Event::Key(key_event)) = event::read() {
            match key_event.code {
                KeyCode::Enter => {
                    print!("\r\n");
                    stdout.flush().ok()?;
                    return Some(line);
                }
                KeyCode::Backspace => {
                    if !line.is_empty() {
                        line.pop();
                        // Move back, print space, move back again
                        print!("\x08 \x08");
                        stdout.flush().ok()?;
                    }
                }
                KeyCode::Char('c')
                    if key_event
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    print!("^C\r\n");
                    stdout.flush().ok()?;
                    return None;
                }
                KeyCode::Char(c) if (0x20..=0x7E).contains(&(c as u8)) => {
                    line.push(c);
                    print!("{}", c);
                    stdout.flush().ok()?;
                }
                // Ignore other keys (arrows, function keys, etc.)
                _ => {}
            }
        }
    }
}

/// Command types for runtime operations
enum Command {
    Insert { drive: DriveNumber, path: String },
    Eject { drive: DriveNumber },
    Resume,
    Quit,
    Help,
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

    Err(anyhow::anyhow!("Invalid command. Type 'help' for usage."))
}

fn show_help() {
    println!("Commands:");
    println!("  load a path/to/disk.img   - Insert disk into drive A:");
    println!("  load b path/to/disk.img   - Insert disk into drive B:");
    println!("  eject a                   - Eject floppy from drive A:");
    println!("  eject b                   - Eject floppy from drive B:");
    println!("  resume (or Enter)         - Resume emulation");
    println!("  quit                      - Halt emulator and exit\r");
}

/// Handle command mode for runtime operations (floppy swapping, etc.)
/// Returns true to continue emulation, false to halt
pub fn handle_command_mode<I, V>(
    computer: &mut Computer<NativeBios<Box<dyn emu86_core::DiskController>>, I, V>,
) -> bool
where
    I: emu86_core::IoDevice,
    V: emu86_core::VideoController,
{
    // Save current screen state
    // We'll use an alternate screen buffer approach: clear screen, show menu,
    // then restore by redrawing the video buffer

    // Use ANSI escape codes to save cursor position and clear screen
    print!("\x1b[s"); // Save cursor position
    print!("\x1b[2J"); // Clear screen
    print!("\x1b[H"); // Move to home position
    io::stdout().flush().ok();

    println!("=== Command Mode (F12) ===\r");
    show_help();

    let should_continue = loop {
        print!("\r\nCommand> ");
        io::stdout().flush().ok();

        let input = match read_line_raw() {
            Some(line) => line,
            None => {
                println!("Cancelled.\r");
                break true; // Continue emulation
            }
        };

        let command = match parse_command(&input) {
            Ok(cmd) => cmd,
            Err(e) => {
                println!("Error: {}\r", e);
                continue;
            }
        };

        match command {
            Command::Help => {
                show_help();
            }
            Command::Resume => {
                println!("Resuming emulation...\r");
                break true;
            }
            Command::Quit => {
                println!("Halting emulator...\r");
                break false;
            }
            Command::Insert { drive, path } => match insert_floppy(computer, drive, &path) {
                Ok(()) => {
                    println!(
                        "Successfully inserted {} into drive {}\r",
                        path,
                        format_drive(drive)
                    );
                    log::info!("Inserted {} into drive 0x{:02X}", path, drive.to_standard());
                }
                Err(e) => {
                    println!("Error: {}\r", e);
                    log::error!("Failed to insert {}: {}", path, e);
                }
            },
            Command::Eject { drive } => match eject_floppy(computer, drive) {
                Ok(()) => {
                    println!("Successfully ejected drive {}\r", format_drive(drive));
                    log::info!("Ejected drive 0x{:02X}", drive.to_standard());
                }
                Err(e) => {
                    println!("Error: {}\r", e);
                    log::error!("Failed to eject: {}", e);
                }
            },
        }
    };

    // Restore screen: clear, then force a full video update
    print!("\x1b[2J"); // Clear screen
    print!("\x1b[H"); // Move to home
    io::stdout().flush().ok();

    // Force video controller to redraw the entire screen
    computer.update_video();

    should_continue
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
