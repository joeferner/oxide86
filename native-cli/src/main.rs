use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use emu86_core::{Computer, NullJoystick};
use emu86_native_common::{
    CommonCli, GilrsJoystick, GilrsJoystickInput, NativeClock, apply_logging_flags,
    attach_serial_device, create_speaker, load_disks, load_program_or_boot,
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
#[command(name = "emu86")]
#[command(about = "Intel 8086 CPU Emulator", long_about = None)]
#[command(
    after_help = "During emulation:\n  Press F12 to enter command mode for floppy swapping and other runtime operations."
)]
struct Cli {
    #[command(flatten)]
    common: CommonCli,
}

fn main() -> Result<()> {
    let log_file = File::create("emu86.log").context("Failed to create log file")?;

    // Initialize logger from RUST_LOG env var, or use defaults if not set
    let mut builder = env_logger::Builder::from_default_env();

    // Only apply defaults if RUST_LOG is not set
    if std::env::var("RUST_LOG").is_err() {
        builder
            .filter_level(log::LevelFilter::Error)
            .filter_module("emu86_core", log::LevelFilter::Info)
            .filter_module("emu86_native", log::LevelFilter::Info);
    }

    builder
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    let default_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        default_panic(info);
    }));

    let cli = Cli::parse();

    // Parse CPU type
    let cpu_type = emu86_core::CpuType::parse(&cli.common.cpu_type)
        .ok_or_else(|| anyhow::anyhow!("Invalid CPU type: {}", cli.common.cpu_type))?;

    // Parse video card type
    let video_card_type = emu86_core::VideoCardType::parse(&cli.common.video_card)
        .ok_or_else(|| anyhow::anyhow!("Invalid video card type: {}", cli.common.video_card))?;

    // Create computer with keyboard, mouse, joystick, video, and speaker
    let keyboard = Box::new(TerminalKeyboard::new());
    let terminal_mouse = TerminalMouse::new();
    let mouse = Box::new(terminal_mouse.clone_shared());

    // Create joystick - only initialize gilrs if joystick flags are set
    let (joystick, mut gilrs_joystick): (
        Box<dyn emu86_core::JoystickInput>,
        Option<GilrsJoystick>,
    ) = if cli.common.joystick_a || cli.common.joystick_b {
        log::info!("Initializing gamepad support (gilrs)");
        let gilrs = GilrsJoystick::new();
        let input = Box::new(GilrsJoystickInput::new(gilrs.clone_state()));
        (input, Some(gilrs))
    } else {
        (Box::new(NullJoystick), None)
    };

    // Initialize speaker BEFORE video so ALSA messages appear before alternate screen
    let speaker = create_speaker();

    // Video init switches to alternate screen - must come after speaker init
    let video = TerminalVideo::new();

    let clock = Box::new(NativeClock);
    let mut computer = Computer::new(
        keyboard,
        mouse,
        joystick,
        clock,
        video,
        speaker,
        emu86_core::ComputerConfig {
            cpu_type,
            memory_kb: cli.common.memory,
            video_card_type,
        },
    );

    // Load disks and program/boot
    load_disks(
        &mut computer,
        &cli.common.floppy_a,
        &cli.common.floppy_b,
        &cli.common.hard_disks,
    )?;
    load_program_or_boot(&mut computer, &cli.common)?;

    // Apply logging flags
    apply_logging_flags(&mut computer, cli.common.exec_log, cli.common.int_log);

    log::info!("Starting execution... (Press F12 for command mode)");

    // Attach serial devices if specified
    if let Some(device) = &cli.common.com1_device {
        let mouse_clone =
            Box::new(terminal_mouse.clone_shared()) as Box<dyn emu86_core::MouseInput>;
        attach_serial_device(&mut computer, 1, device, mouse_clone);
    }
    if let Some(device) = &cli.common.com2_device {
        let mouse_clone =
            Box::new(terminal_mouse.clone_shared()) as Box<dyn emu86_core::MouseInput>;
        attach_serial_device(&mut computer, 2, device, mouse_clone);
    }

    // Enable mouse capture for terminal mouse support
    execute!(std::io::stdout(), EnableMouseCapture).context("Failed to enable mouse capture")?;

    // Run the program with optional speed throttling
    let quit_from_command_mode = {
        let clock_hz = (cli.common.speed * 1_000_000.0) as u64;
        log::info!("Running at {:.2} MHz ({} Hz)", cli.common.speed, clock_hz);
        run(&mut computer, gilrs_joystick.as_mut(), Some(clock_hz))
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

    Ok(())
}

/// Run the emulator with F12 command mode support, mouse input, and joystick polling
/// If clock_hz is Some, throttles to that speed; if None, runs at maximum speed
/// Returns true if user quit from command mode, false if computer halted naturally
fn run<V>(
    computer: &mut Computer<V>,
    mut joystick: Option<&mut GilrsJoystick>,
    clock_hz: Option<u64>,
) -> bool
where
    V: emu86_core::VideoController,
{
    use crossterm::event::{self, Event, MouseButton, MouseEventKind};
    let start_time = clock_hz.map(|_| Instant::now());
    let nanos_per_cycle = clock_hz.map(|hz| 1_000_000_000u64 / hz);

    // Run instructions in batches to reduce timing overhead
    const BATCH_SIZE: u32 = 1000;

    // Track whether we exit from command mode quit
    let mut quit_from_command_mode = false;

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
                        // Convert key event to KeyPress
                        use terminal_keyboard::key_event_to_keypress;
                        let key = key_event_to_keypress(&key_event);

                        // Check if it's F12 (command mode) - intercept for emulator, don't send to program
                        if key.scan_code == terminal_keyboard::SCAN_CODE_F12 {
                            let event = Event::Key(key_event);
                            computer.bios_mut().keyboard.process_event(&event);
                            // Don't fire INT 09h for F12 - it's not visible to the emulated program
                        } else {
                            // Fire INT 09h (keyboard hardware interrupt) for all other keys
                            computer.process_keyboard_irq(key);
                        }
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
