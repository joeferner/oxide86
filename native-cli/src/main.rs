use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use oxide86_core::{Computer, NullJoystick};
use oxide86_native_common::{
    CommonCli, GilrsJoystick, GilrsJoystickInput, NativeClock, apply_logging_flags,
    attach_serial_device, create_audio, load_cdroms, load_disks, load_mounted_directories,
    load_program_or_boot, sync_mounted_directories,
};
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
// MIGRATED      let log_file = File::create("oxide86.log").context("Failed to create log file")?;

// MIGRATED      // Initialize logger from RUST_LOG env var, or use defaults if not set
// MIGRATED      let mut builder = env_logger::Builder::from_default_env();

// MIGRATED      // Only apply defaults if RUST_LOG is not set
// MIGRATED      if std::env::var("RUST_LOG").is_err() {
// MIGRATED          builder
// MIGRATED              .filter_level(log::LevelFilter::Error)
// MIGRATED              .filter_module("oxide86_core", log::LevelFilter::Info)
// MIGRATED              .filter_module("oxide86_native", log::LevelFilter::Info);
// MIGRATED      }

// MIGRATED      builder
// MIGRATED          .target(env_logger::Target::Pipe(Box::new(log_file)))
// MIGRATED          .init();

// MIGRATED      let default_panic = panic::take_hook();
// MIGRATED      panic::set_hook(Box::new(move |info| {
// MIGRATED          let _ = disable_raw_mode();
// MIGRATED          let _ = execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
// MIGRATED          default_panic(info);
// MIGRATED      }));

// MIGRATED      let cli = Cli::parse();

    // Parse CPU type
    let cpu_type = oxide86_core::CpuType::parse(&cli.common.cpu_type)
        .ok_or_else(|| anyhow::anyhow!("Invalid CPU type: {}", cli.common.cpu_type))?;

    // Parse video card type
    let video_card_type = oxide86_core::VideoCardType::parse(&cli.common.video_card)
        .ok_or_else(|| anyhow::anyhow!("Invalid video card type: {}", cli.common.video_card))?;

    // Create computer with keyboard, mouse, joystick, video, and speaker
    let keyboard = Box::new(TerminalKeyboard::new());
    let terminal_mouse = TerminalMouse::new();
    let mouse = Box::new(terminal_mouse.clone_shared());

    // Create joystick - only initialize gilrs if joystick flags are set
    let (joystick, mut gilrs_joystick): (
        Box<dyn oxide86_core::JoystickInput>,
        Option<GilrsJoystick>,
    ) = if cli.common.joystick_a || cli.common.joystick_b {
        log::info!("Initializing gamepad support (gilrs)");
        let gilrs = GilrsJoystick::new();
        let input = Box::new(GilrsJoystickInput::new(gilrs.clone_state()));
        (input, Some(gilrs))
    } else {
        (Box::new(NullJoystick), None)
    };

    // Initialize audio BEFORE video so ALSA messages appear before alternate screen
    let cpu_freq = (cli.common.speed * 1_000_000.0) as u64;
    let (speaker, sound_card, _audio_output) = create_audio(
        !cli.common.disable_pc_speaker,
        &cli.common.sound_card,
        cpu_freq,
    );

    // Video init switches to alternate screen - must come after audio init
    let video = TerminalVideo::new();

    let clock = Box::new(NativeClock);
    let mut computer = Computer::new(
        keyboard,
        mouse,
        joystick,
        clock,
        video,
        speaker,
        oxide86_core::ComputerConfig {
            cpu_type,
            memory_kb: cli.common.memory,
            video_card_type,
            cpu_freq,
        },
    );

    // Connect sound card if available.
    if let Some(card) = sound_card {
        computer.set_sound_card(card);
    }

    // Load disks and program/boot
    load_disks(
        &mut computer,
        &cli.common.floppy_a,
        &cli.common.floppy_b,
        &cli.common.hard_disks,
    )?;

    // Load CD-ROM images
    load_cdroms(&mut computer, &cli.common.cdroms)?;

    // Load mounted directories
    let mounted_drives = load_mounted_directories(&mut computer, &cli.common.mount_dirs)?;

    load_program_or_boot(&mut computer, &cli.common)?;

    // Apply logging flags
    apply_logging_flags(&mut computer, cli.common.exec_log, cli.common.int_log);

    log::info!("Starting execution... (Press F12 for command mode)");

    // Attach serial devices if specified
    if let Some(device) = &cli.common.com1_device {
        let mouse_clone =
            Box::new(terminal_mouse.clone_shared()) as Box<dyn oxide86_core::MouseInput>;
        attach_serial_device(&mut computer, 1, device, mouse_clone);
    }
    if let Some(device) = &cli.common.com2_device {
        let mouse_clone =
            Box::new(terminal_mouse.clone_shared()) as Box<dyn oxide86_core::MouseInput>;
        attach_serial_device(&mut computer, 2, device, mouse_clone);
    }

    // Enable mouse capture for terminal mouse support
    execute!(std::io::stdout(), EnableMouseCapture).context("Failed to enable mouse capture")?;

    // Run the program with optional speed throttling
    let quit_from_command_mode = {
        let cpu_freq = (cli.common.speed * 1_000_000.0) as u64;
        log::info!("Running at {:.2} MHz ({} Hz)", cli.common.speed, cpu_freq);
        run(&mut computer, gilrs_joystick.as_mut(), Some(cpu_freq))
    };

    // Disable mouse capture before exiting
    execute!(std::io::stdout(), DisableMouseCapture).context("Failed to disable mouse capture")?;

    log::info!("=== Execution complete ===");
    computer.dump_registers();

    // If computer halted naturally (not from command mode quit), wait for keypress
    if !quit_from_command_mode {
        use crossterm::event::{self, Event};
        use crossterm::style::Print;
        execute!(std::io::stdout(), Print("\nPress any key to exit..."))?;
        loop {
            if let Ok(Event::Key(key_event)) = event::read() {
                // Exit on any key press
                if let crossterm::event::KeyEventKind::Press = key_event.kind {
                    break;
                }
            }
        }
    }

    // Sync mounted directories before exit
    sync_mounted_directories(&mut computer, &mounted_drives)?;

    Ok(())
}

/// Run the emulator with F12 command mode support, mouse input, and joystick polling
/// If cpu_freq is Some, throttles to that speed; if None, runs at maximum speed
/// Returns true if user quit from command mode, false if computer halted naturally
fn run<V>(
    computer: &mut Computer<V>,
    mut joystick: Option<&mut GilrsJoystick>,
    cpu_freq: Option<u64>,
) -> bool
where
    V: oxide86_core::VideoController,
{
    use crossterm::event::{self, Event, MouseButton, MouseEventKind};
    let start_time = cpu_freq.map(|_| Instant::now());
    let nanos_per_cycle = cpu_freq.map(|hz| 1_000_000_000u64 / hz);

    // Run instructions in batches to reduce timing overhead
    const BATCH_SIZE: u32 = 1000;

    // Track whether we exit from command mode quit
    let mut quit_from_command_mode = false;

    while !computer.is_terminal_halt() {
        // Execute a batch of instructions
        for _ in 0..BATCH_SIZE {
            if computer.is_terminal_halt() {
                break;
            }
            computer.step();
        }

        // Update video once per batch instead of after every instruction
        // This dramatically reduces terminal I/O overhead
        computer.update_video();

        // Poll for joystick events (gamepad input) if enabled
        if let Some(ref mut js) = joystick {
            js.poll();
        }

        // Poll for events (keyboard and mouse) without blocking
        // Process all available events to keep input responsive
        while event::poll(Duration::from_millis(0)).unwrap_or(false) {
            if let Ok(ev) = event::read() {
                match ev {
                    Event::Key(key_event) => {
                        // MIGRATED
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
                quit_from_command_mode = true;
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

    quit_from_command_mode
}
