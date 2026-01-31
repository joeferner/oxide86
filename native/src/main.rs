use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use emu86_core::utils::parse_hex_or_dec;
use emu86_core::{BackedDisk, Computer, DiskController, DriveNumber, FileDiskBackend};
use std::fs::File;
use std::panic;
use std::time::{Duration, Instant};

mod terminal_keyboard;
use terminal_keyboard::TerminalKeyboard;

mod terminal_video;
use terminal_video::TerminalVideo;

mod terminal_mouse;
use terminal_mouse::TerminalMouse;

mod command_mode;

#[derive(Parser)]
#[command(name = "emu86")]
#[command(about = "Intel 8086 CPU Emulator", long_about = None)]
#[command(
    after_help = "During emulation:\n  Press F12 to enter command mode for floppy swapping and other runtime operations."
)]
struct Cli {
    /// Path to the program binary to load and execute (not used with --boot)
    #[arg(required_unless_present = "boot")]
    program: Option<String>,

    /// Boot from disk image instead of loading a program
    #[arg(long)]
    boot: bool,

    /// Boot drive number (0x00 for floppy A:, 0x01 for floppy B:, 0x80 for hard disk C:)
    #[arg(long, default_value = "0x00")]
    boot_drive: String,

    /// Starting segment address (default: 0x0000)
    #[arg(long, default_value = "0x0000")]
    segment: String,

    /// Starting offset address (default: 0x0100, like .COM files)
    #[arg(long, default_value = "0x0100")]
    offset: String,

    /// Path to disk image file for floppy A:
    #[arg(long = "floppy-a")]
    floppy_a: Option<String>,

    /// Path to disk image file for floppy B:
    #[arg(long = "floppy-b")]
    floppy_b: Option<String>,

    /// Path to hard disk image file(s) - can be specified multiple times for C:, D:, etc.
    #[arg(long = "hdd", action = clap::ArgAction::Append)]
    hard_disks: Vec<String>,

    /// CPU clock speed in MHz (default: 4.77 for original 8086)
    #[arg(long, default_value = "4.77")]
    speed: f64,

    /// Run at maximum speed (no throttling)
    #[arg(long)]
    turbo: bool,
}

fn main() -> Result<()> {
    let log_file = File::create("/tmp/emu86.log").context("Failed to create log file")?;
    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    let default_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        default_panic(info);
    }));

    let cli = Cli::parse();

    // Create computer with keyboard and mouse
    let keyboard = TerminalKeyboard::new();
    let mouse = Box::new(TerminalMouse::new());
    let video = TerminalVideo::new();
    let mut computer = Computer::new(keyboard, mouse, video);

    // Load floppy A:
    if let Some(path) = &cli.floppy_a {
        let backend = FileDiskBackend::open(path, false)?;
        let disk = BackedDisk::new(backend)
            .with_context(|| format!("Failed to create disk from: {}", path))?;
        computer
            .bios_mut()
            .insert_floppy(DriveNumber::floppy_a(), Box::new(disk))
            .map_err(|e| anyhow::anyhow!("Failed to insert floppy A:: {}", e))?;
        log::info!("Opened floppy A: from {}", path);
    }

    // Load floppy B:
    if let Some(path) = &cli.floppy_b {
        let backend = FileDiskBackend::open(path, false)?;
        let disk = BackedDisk::new(backend)
            .with_context(|| format!("Failed to create disk from: {}", path))?;
        computer
            .bios_mut()
            .insert_floppy(DriveNumber::floppy_b(), Box::new(disk))
            .map_err(|e| anyhow::anyhow!("Failed to insert floppy B:: {}", e))?;
        log::info!("Opened floppy B: from {}", path);
    }

    // Load hard drives (C:, D:, etc.)
    for path in cli.hard_disks.iter() {
        let backend = FileDiskBackend::open(path, false)?;
        let disk = BackedDisk::new(backend)
            .with_context(|| format!("Failed to create disk from: {}", path))?;

        // Check if disk has MBR and partitions
        use emu86_core::parse_mbr;
        let sector_0 = disk.read_sector_lba(0).ok();
        let has_partitions = sector_0
            .as_ref()
            .and_then(parse_mbr)
            .and_then(|parts| parts[0]);

        let drive_num = if let Some(partition) = has_partitions {
            log::info!(
                "Detected MBR on {}: partition 1 at sector {}, {} sectors",
                path,
                partition.start_sector,
                partition.sector_count
            );
            // Open the file again for raw disk access
            let raw_backend = FileDiskBackend::open(path, false)?;
            let raw_disk = BackedDisk::new(raw_backend)
                .with_context(|| format!("Failed to create raw disk from: {}", path))?;

            use emu86_core::PartitionedDisk;
            let partitioned =
                PartitionedDisk::new(disk, partition.start_sector, partition.sector_count);
            computer
                .bios_mut()
                .add_hard_drive_with_partition(Box::new(partitioned), Box::new(raw_disk))
        } else {
            log::info!("No MBR detected on {}, using raw disk", path);
            computer.bios_mut().add_hard_drive(Box::new(disk))
        };

        log::info!(
            "Opened hard drive {}: ({}) from {}",
            drive_num.to_letter(),
            drive_num,
            path
        );
    }

    // If no drives specified and booting, error out
    if cli.floppy_a.is_none() && cli.floppy_b.is_none() && cli.hard_disks.is_empty() && cli.boot {
        return Err(anyhow::anyhow!(
            "No disk images specified. Use --floppy-a, --floppy-b, or --hdd to specify disk images."
        ));
    }

    if cli.boot {
        // Boot from disk
        let boot_drive = parse_hex_or_dec(&cli.boot_drive)?;
        if boot_drive > 0xFF {
            return Err(anyhow::anyhow!(
                "Boot drive must be 0x00-0xFF, got 0x{:04X}",
                boot_drive
            ));
        }
        let boot_drive = DriveNumber::from_standard(boot_drive as u8);

        log::info!("Booting from drive {}...", boot_drive);
        computer
            .boot(boot_drive)
            .context("Failed to boot from disk")?;

        log::info!("Boot sector loaded at 0x0000:0x7C00");
        log::info!("Starting execution... (Press F12 for command mode)\n");
    } else {
        // Load program from file
        let program_path = cli.program.as_ref().unwrap(); // Safe because of required_unless_present
        let program_data = std::fs::read(program_path)
            .with_context(|| format!("Failed to read program file: {}", program_path))?;

        let segment = parse_hex_or_dec(&cli.segment)?;
        let offset = parse_hex_or_dec(&cli.offset)?;

        computer
            .load_program(&program_data, segment, offset)
            .context("Failed to load program")?;

        log::info!(
            "Loaded {} bytes at {:04X}:{:04X}",
            program_data.len(),
            segment,
            offset
        );
        log::info!("Starting execution... (Press F12 for command mode)\n");
    }

    // Enable mouse capture for terminal mouse support
    execute!(std::io::stdout(), EnableMouseCapture).context("Failed to enable mouse capture")?;

    // Run the program with optional speed throttling
    if cli.turbo {
        log::info!("Running in turbo mode (no speed limit)");
        run(&mut computer, None);
    } else {
        let clock_hz = (cli.speed * 1_000_000.0) as u64;
        log::info!("Running at {:.2} MHz ({} Hz)", cli.speed, clock_hz);
        run(&mut computer, Some(clock_hz));
    }

    // Disable mouse capture before exiting
    execute!(std::io::stdout(), DisableMouseCapture).context("Failed to disable mouse capture")?;

    log::info!("=== Execution complete ===");
    computer.dump_registers();

    Ok(())
}

/// Run the emulator with F12 command mode support and mouse input
/// Specific to TerminalKeyboard to support F12 command mode detection
/// If clock_hz is Some, throttles to that speed; if None, runs at maximum speed
fn run<V>(computer: &mut Computer<TerminalKeyboard, V>, clock_hz: Option<u64>)
where
    V: emu86_core::VideoController,
{
    use crossterm::event::{self, Event, MouseButton, MouseEventKind};
    let start_time = clock_hz.map(|_| Instant::now());
    let nanos_per_cycle = clock_hz.map(|hz| 1_000_000_000u64 / hz);

    // Run instructions in batches to reduce timing overhead
    const BATCH_SIZE: u32 = 1000;

    while !computer.is_halted() {
        // Execute a batch of instructions
        for _ in 0..BATCH_SIZE {
            if computer.is_halted() {
                break;
            }
            computer.step();
        }

        // Update video once per batch instead of after every instruction
        // This dramatically reduces terminal I/O overhead
        computer.update_video();

        // Poll for events (keyboard and mouse) without blocking
        // Process all available events to keep input responsive
        while event::poll(Duration::from_millis(0)).unwrap_or(false) {
            if let Ok(ev) = event::read() {
                match ev {
                    Event::Key(_) => {
                        // Dispatch keyboard event to keyboard handler
                        computer.bios_mut().keyboard.process_event(ev);
                    }
                    Event::Mouse(mouse_event) => {
                        // Dispatch mouse events to mouse handler
                        match mouse_event.kind {
                            MouseEventKind::Moved => {
                                computer.bios_mut().mouse.process_cursor_moved(
                                    mouse_event.column as f64,
                                    mouse_event.row as f64,
                                );
                            }
                            MouseEventKind::Down(button) => {
                                let button_code = match button {
                                    MouseButton::Left => 0,
                                    MouseButton::Right => 1,
                                    MouseButton::Middle => 2,
                                };
                                computer.bios_mut().mouse.process_button(button_code, true);
                            }
                            MouseEventKind::Up(button) => {
                                let button_code = match button {
                                    MouseButton::Left => 0,
                                    MouseButton::Right => 1,
                                    MouseButton::Middle => 2,
                                };
                                computer.bios_mut().mouse.process_button(button_code, false);
                            }
                            MouseEventKind::Drag(_) => {
                                // Drag events also update position
                                computer.bios_mut().mouse.process_cursor_moved(
                                    mouse_event.column as f64,
                                    mouse_event.row as f64,
                                );
                            }
                            _ => {} // Ignore scroll and other events
                        }
                    }
                    _ => {} // Ignore resize and other events
                }
            }
        }

        // Check if F12 was pressed (intercepted by BIOS)
        if computer.bios_mut().keyboard.is_command_mode_requested() {
            computer.bios_mut().keyboard.clear_command_mode_request();
            log::info!("F12 detected - entering command mode");

            // Enter command mode and check if we should continue
            let should_continue =
                command_mode::handle_command_mode(computer).unwrap_or_else(|err| {
                    log::error!("failed to handle command mode: {err}");
                    true
                });
            if !should_continue {
                // User chose to quit
                break;
            }
        }

        // Throttle if clock speed is specified
        if let (Some(start), Some(nanos)) = (start_time, nanos_per_cycle) {
            // Calculate how much time should have elapsed
            let cycles_executed = computer.get_cycle_count();
            let expected_nanos = cycles_executed * nanos;
            let expected_duration = Duration::from_nanos(expected_nanos);

            // Sleep if we're running ahead of schedule
            let actual_elapsed = start.elapsed();
            if actual_elapsed < expected_duration {
                let sleep_duration = expected_duration - actual_elapsed;
                // Only sleep if it's worth it (> 100 microseconds)
                if sleep_duration > Duration::from_micros(100) {
                    std::thread::sleep(sleep_duration);
                }
            }
        }
    }
}
