use anyhow::Context;
use anyhow::Result;
use chrono::Datelike;
use chrono::Timelike;
use clap::Parser;
use menu::AppMenu;
use oxide86_core::computer::Computer;
use oxide86_core::devices::serial_mouse::SerialMouse;
use oxide86_core::disk::BackedDisk;
use oxide86_core::disk::DriveNumber;
use oxide86_core::scan_code::{
    SCAN_CODE_E1_PREFIX, SCAN_CODE_EXTENDED_PREFIX, SCAN_CODE_LEFT_ALT, SCAN_CODE_LEFT_ALT_RELEASE,
    SCAN_CODE_LEFT_CTRL, SCAN_CODE_LEFT_CTRL_RELEASE, SCAN_CODE_RELEASE,
};
use oxide86_core::video::TEXT_MODE_COLS;
use oxide86_core::video::TEXT_MODE_ROWS;
use oxide86_core::video::VideoBuffer;
use oxide86_core::video::font::CHAR_HEIGHT;
use oxide86_core::video::font::CHAR_WIDTH;
use oxide86_native_common::cli::CommonCli;
use oxide86_native_common::create_computer;
use oxide86_native_common::disk::FileDiskBackend;
use oxide86_native_common::has_com_mouse;
use oxide86_native_common::logging::setup_logging;
use oxide86_native_common::throttle::CpuThrottle;
use pixels::{Pixels, SurfaceTexture, wgpu};
use rodio::MixerDeviceSink;
use std::sync::Mutex;
use std::sync::{Arc, RwLock};
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, KeyLocation, NamedKey, PhysicalKey};
use winit::window::{CursorGrabMode, WindowBuilder};

use crate::mouse_motion_state::MouseMotionState;
use crate::notification::{Notification, NotificationType};
use crate::performance_tracker::PerformanceTracker;

mod menu;
mod mouse_motion_state;
mod notification;
mod performance_tracker;

const TITLE: &str = "Oxide86 - x86 Emulator";
/// Screen dimensions in pixels
const SCREEN_WIDTH: usize = TEXT_MODE_COLS * CHAR_WIDTH; // 640
const SCREEN_HEIGHT: usize = TEXT_MODE_ROWS * CHAR_HEIGHT; // 400

#[derive(Parser)]
#[command(name = "oxide86-gui")]
#[command(about = "Intel 8086 CPU Emulator (GUI)", long_about = None)]
struct Cli {
    #[command(flatten)]
    common: CommonCli,
}

struct AppState {
    menu: AppMenu,
    floppy_a_present: bool,
    floppy_b_present: bool,
    // TODO cdrom_present: bool,
    is_paused: bool,
    show_performance_overlay: bool,
    perf_tracker: PerformanceTracker,
    notification: Option<Notification>,
    halted: bool,
    target_mhz: f64,
    audio_sink: Option<Mutex<MixerDeviceSink>>,
}

fn main() -> Result<()> {
    setup_logging()?;

    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        log::error!("Application error: {:#}", e);
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
    Ok(())
}

fn run(cli: Cli) -> Result<()> {
    let event_loop = EventLoop::new().context("Failed to create event loop")?;

    let icon = load_icon();

    let window = WindowBuilder::new()
        .with_title(TITLE)
        .with_window_icon(icon)
        .with_inner_size(LogicalSize::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32))
        .with_resizable(true)
        .build(&event_loop)
        .context("Failed to create window")?;

    let (window, mut pixels) = create_window_and_pixels(window)?;

    let com_has_mouse = has_com_mouse(&cli.common)?;
    let ps2_mouse = cli.common.ps2_mouse;
    let video_buffer = Arc::new(RwLock::new(VideoBuffer::new()));
    let serial_mouse = if com_has_mouse {
        Some(Arc::new(RwLock::new(SerialMouse::new())))
    } else {
        None
    };

    // Initialize computer
    let (mut computer, audio_sink, mut gilrs_joystick) =
        create_computer(&cli.common, video_buffer.clone(), serial_mouse.clone())?;

    // Initialize egui
    let (egui_ctx, mut egui_state, mut egui_renderer) = setup_egui(window, &pixels);

    // Create application state
    let mut app_state = AppState {
        menu: AppMenu::new(),
        floppy_a_present: cli.common.floppy_a.is_some(),
        floppy_b_present: cli.common.floppy_b.is_some(),
        // TODO cdrom_present: !cli.common.cdroms.is_empty(),
        is_paused: false,
        show_performance_overlay: false,
        perf_tracker: PerformanceTracker::new(),
        notification: None,
        halted: false,
        target_mhz: cli.common.speed,
        audio_sink: audio_sink.map(Mutex::new),
    };
    // TODO app_state.menu.update_cdrom_state(app_state.cdrom_present);

    let clock_speed = computer.get_clock_speed();
    let max_cycles_per_frame = (clock_speed as u64 / 40).max(100_000);
    let mut throttle = CpuThrottle::new(clock_speed, computer.get_cycle_count());
    let mut was_paused = false;

    log::info!(
        "Running at {:.2} MHz ({} Hz)",
        cli.common.speed,
        clock_speed
    );

    // Update menu states
    app_state
        .menu
        .update_menu_states(app_state.floppy_a_present, app_state.floppy_b_present);

    // Exclusive mode state - when true, hides cursor and disables menu
    let mut exclusive_mode = false;
    // Track cursor position to detect clicks in menu area
    let mut cursor_y = 0.0;

    let mut mouse_motion_state = MouseMotionState::new();

    // Track actual surface size (may differ from window size due to aspect ratio adjustment)
    let mut surface_size = window.inner_size();

    // Mouse button state for PS/2 and serial mouse
    let mut mouse_left = false;
    let mut mouse_right = false;

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            // Handle device events for raw mouse input (only when cursor is truly locked)
            if let Event::DeviceEvent { event, .. } = &event
                && let DeviceEvent::MouseMotion { delta } = event
                && exclusive_mode
            {
                let (actual_delta_x, actual_delta_y) = mouse_motion_state.process_motion(*delta);

                if ps2_mouse {
                    let buttons = (mouse_left as u8) | ((mouse_right as u8) << 1);
                    computer.push_ps2_mouse_event(
                        actual_delta_x.clamp(-128.0, 127.0) as i8,
                        actual_delta_y.clamp(-128.0, 127.0) as i8,
                        buttons,
                    );
                } else if let Some(ref m) = serial_mouse {
                    m.write()
                        .unwrap()
                        .push_motion(actual_delta_x as i16, actual_delta_y as i16);
                }
            }

            if let Event::WindowEvent { event, .. } = event {
                // Track cursor position
                if let WindowEvent::CursorMoved { position, .. } = &event {
                    cursor_y = position.y;
                }

                // In exclusive mode, skip egui event handling
                // When not in exclusive mode, let egui handle events first
                if !exclusive_mode {
                    let response = egui_state.on_window_event(window, &event);
                    // Keyboard events always reach the emulator — egui's overlay has no
                    // text inputs that should steal from the emulated program.
                    // Non-keyboard events (e.g., clicking on the menu) are blocked if egui
                    // consumed them.
                    if response.consumed && !matches!(event, WindowEvent::KeyboardInput { .. }) {
                        return;
                    }
                }

                // Check for mouse click to enter exclusive mode
                // Don't enter if cursor is in menu bar area (top ~30 pixels)
                const MENU_HEIGHT: f64 = 30.0;
                if !exclusive_mode
                    && cursor_y > MENU_HEIGHT
                    && let WindowEvent::MouseInput { state, .. } = &event
                    && *state == ElementState::Pressed
                {
                    exclusive_mode = true;
                    window.set_cursor_visible(false);
                    window.set_title(&format!("{} (F12 to unlock cursor)", TITLE));

                    // Try to lock cursor for true relative motion
                    if window.set_cursor_grab(CursorGrabMode::Locked).is_ok() {
                        log::info!(
                            "Cursor locked successfully - using DeviceEvent for mouse input"
                        );
                    } else {
                        log::info!(
                            "Cursor lock not supported - using WindowEvent with manual tracking"
                        );
                        // Try confined mode as fallback to keep cursor in window
                        let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                    }

                    log::info!("Entered exclusive mode (press F12 to exit)");
                    // Don't return - let the event continue to be processed
                }

                match event {
                    WindowEvent::CloseRequested => {
                        log::info!("Window close requested");
                        computer.log_cpu_state();
                        // use sink down here to make sure it doesn't get dropped
                        if let Some(audio_sink) = &app_state.audio_sink {
                            audio_sink.lock().unwrap().log_on_drop(false);
                        }
                        std::process::exit(0);
                    }
                    WindowEvent::Resized(new_size) => {
                        handle_window_resize(&mut pixels, new_size, &mut surface_size);
                    }
                    WindowEvent::KeyboardInput { event: input, .. } => {
                        // Always handle keyboard input (F12 to exit exclusive mode, etc.)
                        // Even when halted, F12 should still unlock the cursor
                        handle_keyboard_input(&mut computer, window, &input, &mut exclusive_mode);
                    }
                    WindowEvent::ModifiersChanged(_) => {
                        // Modifier state is handled through KeyboardInput make/break scan codes
                    }
                    WindowEvent::MouseInput { button, state, .. } => {
                        if exclusive_mode {
                            handle_mouse_button(
                                &mut computer,
                                &serial_mouse,
                                ps2_mouse,
                                button,
                                state,
                                &mut mouse_left,
                                &mut mouse_right,
                            );
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        // Poll joystick for gamepad events if enabled
                        if let Some(ref mut js) = gilrs_joystick {
                            js.poll(&mut computer.joystick_mut());
                        }

                        let any_paused = app_state.is_paused || computer.is_debug_paused();

                        // Release/re-grab cursor when pause state changes in exclusive mode
                        if exclusive_mode {
                            if !was_paused && any_paused {
                                exclusive_mode = false;
                                window.set_cursor_visible(true);
                                let _ = window.set_cursor_grab(CursorGrabMode::None);
                                window.set_title(TITLE);
                            } else if was_paused && !any_paused {
                                window.set_cursor_visible(false);
                                if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                                    let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                                }
                            }
                        }

                        // Reset throttle when resuming from pause to avoid burst catch-up
                        if was_paused && !any_paused {
                            throttle.reset(computer.get_cycle_count());
                        }
                        was_paused = any_paused;

                        let halted = step_emulator(
                            &mut computer,
                            &mut pixels,
                            &video_buffer,
                            any_paused || app_state.halted,
                            &mut throttle,
                            max_cycles_per_frame,
                        );

                        // Handle halt: show notification and exit exclusive mode
                        if halted && !app_state.halted {
                            app_state.halted = true;
                            app_state.notification = Some(Notification::new(
                                "Program terminated. Close window to exit.".to_string(),
                                NotificationType::Success,
                            ));
                            // Exit exclusive mode so the cursor and menu are visible
                            if exclusive_mode {
                                exclusive_mode = false;
                                window.set_cursor_visible(true);
                                let _ = window.set_cursor_grab(CursorGrabMode::None);
                            }
                            window.set_title(&format!("{} [Terminated]", TITLE));
                        }

                        // Update performance tracker
                        app_state.perf_tracker.update(computer.get_cycle_count());

                        let full_output = process_egui_frame(
                            &egui_ctx,
                            &mut egui_state,
                            window,
                            exclusive_mode,
                            &mut app_state,
                            &mut computer,
                            &video_buffer,
                        );

                        render_frame(
                            &mut pixels,
                            &mut egui_renderer,
                            &egui_ctx,
                            full_output,
                            window,
                            surface_size,
                        );

                        window.request_redraw();
                    }
                    _ => {}
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("Event loop error: {}", e))
}

fn load_icon() -> Option<winit::window::Icon> {
    let icon_bytes = include_bytes!("../assets/logo.png");
    image::load_from_memory(icon_bytes)
        .ok()
        .map(|img| img.into_rgba8())
        .and_then(|rgba| {
            let (w, h) = rgba.dimensions();
            winit::window::Icon::from_rgba(rgba.into_raw(), w, h).ok()
        })
}

fn setup_egui(
    window: &'static winit::window::Window,
    pixels: &Pixels,
) -> (egui::Context, egui_winit::State, egui_wgpu::Renderer) {
    let egui_ctx = egui::Context::default();
    let egui_state =
        egui_winit::State::new(egui_ctx.clone(), egui::ViewportId::ROOT, window, None, None);

    let device = pixels.device();
    let target_format = pixels.render_texture_format();
    let egui_renderer = egui_wgpu::Renderer::new(device, target_format, None, 1);

    (egui_ctx, egui_state, egui_renderer)
}

fn create_window_and_pixels(
    window: winit::window::Window,
) -> Result<(&'static winit::window::Window, Pixels<'static>)> {
    // Leak window to get a 'static reference for the event loop
    let window: &'static _ = Box::leak(Box::new(window));
    let window_size = window.inner_size();

    // Create pixels surface
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, window);
    let pixels = Pixels::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface_texture)
        .context("Failed to create Pixels")?;

    Ok((window, pixels))
}

fn handle_window_resize(
    pixels: &mut Pixels,
    new_size: winit::dpi::PhysicalSize<u32>,
    surface_size: &mut winit::dpi::PhysicalSize<u32>,
) {
    // Validate dimensions are non-zero
    if new_size.width == 0 || new_size.height == 0 {
        log::warn!(
            "Ignoring invalid window resize: {}x{}",
            new_size.width,
            new_size.height
        );
        return;
    }

    // Maintain aspect ratio (8:5 for 640:400) to avoid viewport mismatches
    let aspect_ratio = SCREEN_WIDTH as f32 / SCREEN_HEIGHT as f32;
    let new_height = (new_size.width as f32 / aspect_ratio).round() as u32;
    let adjusted_size = winit::dpi::PhysicalSize::new(new_size.width, new_height);

    if let Err(e) = pixels.resize_surface(adjusted_size.width, adjusted_size.height) {
        log::error!(
            "Failed to resize surface to {}x{}: {}",
            adjusted_size.width,
            adjusted_size.height,
            e
        );
        return;
    }

    *surface_size = adjusted_size;
}

fn handle_keyboard_input(
    computer: &mut Computer,
    window: &winit::window::Window,
    input: &winit::event::KeyEvent,
    exclusive_mode: &mut bool,
) {
    log::debug!("KeyboardInput {input:?}");

    // Check if F12 is pressed to exit exclusive mode
    if input.state == ElementState::Pressed
        && let PhysicalKey::Code(KeyCode::F12) = input.physical_key
        && *exclusive_mode
    {
        *exclusive_mode = false;
        window.set_cursor_visible(true);
        window.set_title(TITLE);

        if let Err(e) = window.set_cursor_grab(CursorGrabMode::None) {
            log::warn!("Failed to release cursor grab: {}", e);
        }

        log::info!("Exited exclusive mode (F12)");
        return;
    }

    // Convert physical key to XT scan code and send to emulator.
    // Repeat events are forwarded as additional make codes — this matches real
    // hardware where the keyboard controller generates typematic repeats.
    // Key-release events are never repeated by the OS, so no break code duplication occurs.

    // Right Alt and Right Ctrl are extended keys: send E0 prefix + scan code.
    // Physical key codes on Linux are unreliable (e.g. RightAlt reports as ArrowLeft,
    // RightCtrl as ControlRight for PageDown, etc.), so identify solely by logical
    // key + location.
    let is_right_alt = matches!(&input.logical_key, Key::Named(NamedKey::Alt))
        && input.location == KeyLocation::Right;
    let is_right_ctrl = matches!(&input.logical_key, Key::Named(NamedKey::Control))
        && input.location == KeyLocation::Right;
    // Pause key: real hardware sends E1 1D 45 (press only, no break code).
    if matches!(&input.logical_key, Key::Named(NamedKey::Pause)) {
        if input.state == ElementState::Pressed {
            computer.push_key_press(SCAN_CODE_E1_PREFIX);
            computer.push_key_press(0x1D);
            computer.push_key_press(0x45);
        }
        return;
    }

    if is_right_alt || is_right_ctrl {
        let (make, brk) = if is_right_alt {
            (SCAN_CODE_LEFT_ALT, SCAN_CODE_LEFT_ALT_RELEASE)
        } else {
            (SCAN_CODE_LEFT_CTRL, SCAN_CODE_LEFT_CTRL_RELEASE)
        };
        computer.push_key_press(SCAN_CODE_EXTENDED_PREFIX);
        match input.state {
            ElementState::Pressed => computer.push_key_press(make),
            ElementState::Released => computer.push_key_press(brk),
        }
        return;
    }

    let scan_code = keycode_to_scan_code(input);
    if scan_code != 0 {
        // Keys that share scan codes with other keys but require the E0 extended prefix:
        //   - Dedicated cursor cluster (arrows, Home/End/PgUp/PgDn/Ins/Del): same scan codes
        //     as numpad, distinguished by location: Standard vs Numpad.
        //   - Numpad Enter (0xE0 0x1C) vs main Enter (0x1C): location: Numpad.
        let needs_extended_prefix = (input.location == KeyLocation::Standard
            && matches!(
                scan_code,
                0x47 | 0x48 | 0x49 | 0x4B | 0x4D | 0x4F | 0x50 | 0x51 | 0x52 | 0x53
            ))
            || (input.location == KeyLocation::Numpad && matches!(scan_code, 0x1C | 0x35));
        if needs_extended_prefix {
            computer.push_key_press(SCAN_CODE_EXTENDED_PREFIX);
        }
        match input.state {
            ElementState::Pressed => computer.push_key_press(scan_code),
            ElementState::Released => computer.push_key_press(SCAN_CODE_RELEASE | scan_code),
        }
    }
}

/// Map a winit KeyEvent to an IBM PC XT scan code.
/// Tries the physical key first; falls back to logical key for non-standard layouts.
/// Exception: numpad character keys (NumLock on) are matched by logical key first because
/// the physical key codes reported by the OS for numpad keys are unreliable on some Linux setups.
fn keycode_to_scan_code(input: &winit::event::KeyEvent) -> u8 {
    // Handle keys whose physical code is wrong on some Linux setups before the physical lookup.
    if let Key::Named(NamedKey::PrintScreen) = &input.logical_key {
        return 0x54;
    }

    if input.location == KeyLocation::Numpad
        && let Key::Character(c) = &input.logical_key
    {
        return match c.as_str() {
            "/" => 0x35,
            "*" => 0x37,
            "-" => 0x4A,
            "+" => 0x4E,
            "5" => 0x4C,
            _ => 0x00,
        };
    }
    if let PhysicalKey::Code(key_code) = input.physical_key {
        let sc = physical_keycode_to_scan_code(key_code);
        if sc != 0 {
            return sc;
        }
    }
    match &input.logical_key {
        Key::Named(NamedKey::Escape) => 0x01,
        Key::Named(NamedKey::Backspace) => 0x0E,
        Key::Named(NamedKey::Tab) => 0x0F,
        Key::Named(NamedKey::Enter) => 0x1C,
        Key::Named(NamedKey::CapsLock) => 0x3A,
        Key::Named(NamedKey::F1) => 0x3B,
        Key::Named(NamedKey::F2) => 0x3C,
        Key::Named(NamedKey::F3) => 0x3D,
        Key::Named(NamedKey::F4) => 0x3E,
        Key::Named(NamedKey::F5) => 0x3F,
        Key::Named(NamedKey::F6) => 0x40,
        Key::Named(NamedKey::F7) => 0x41,
        Key::Named(NamedKey::F8) => 0x42,
        Key::Named(NamedKey::F9) => 0x43,
        Key::Named(NamedKey::F10) => 0x44,
        Key::Named(NamedKey::NumLock) => 0x45,
        Key::Named(NamedKey::ScrollLock) => 0x46,
        Key::Named(NamedKey::Home) => 0x47,
        Key::Named(NamedKey::ArrowUp) => 0x48,
        Key::Named(NamedKey::PageUp) => 0x49,
        Key::Named(NamedKey::ArrowLeft) => 0x4B,
        Key::Named(NamedKey::ArrowRight) => 0x4D,
        Key::Named(NamedKey::End) => 0x4F,
        Key::Named(NamedKey::ArrowDown) => 0x50,
        Key::Named(NamedKey::PageDown) => 0x51,
        Key::Named(NamedKey::Insert) => 0x52,
        Key::Named(NamedKey::Delete) => 0x53,
        Key::Named(NamedKey::F11) => 0x57,
        Key::Named(NamedKey::F12) => 0x58,
        Key::Named(NamedKey::Space) => 0x39,
        Key::Named(NamedKey::Alt) => 0x38,
        Key::Named(NamedKey::Control) => 0x1D,
        Key::Named(NamedKey::Shift) => 0x2A,
        _ => {
            log::warn!(
                "unhandled key: physical={:?} logical={:?}",
                input.physical_key,
                input.logical_key
            );
            0x00
        }
    }
}

/// Map a winit physical KeyCode to an IBM PC XT scan code.
fn physical_keycode_to_scan_code(key: KeyCode) -> u8 {
    match key {
        KeyCode::Escape => 0x01,
        KeyCode::Digit1 => 0x02,
        KeyCode::Digit2 => 0x03,
        KeyCode::Digit3 => 0x04,
        KeyCode::Digit4 => 0x05,
        KeyCode::Digit5 => 0x06,
        KeyCode::Digit6 => 0x07,
        KeyCode::Digit7 => 0x08,
        KeyCode::Digit8 => 0x09,
        KeyCode::Digit9 => 0x0A,
        KeyCode::Digit0 => 0x0B,
        KeyCode::Minus => 0x0C,
        KeyCode::Equal => 0x0D,
        KeyCode::Backspace => 0x0E,
        KeyCode::Tab => 0x0F,
        KeyCode::KeyQ => 0x10,
        KeyCode::KeyW => 0x11,
        KeyCode::KeyE => 0x12,
        KeyCode::KeyR => 0x13,
        KeyCode::KeyT => 0x14,
        KeyCode::KeyY => 0x15,
        KeyCode::KeyU => 0x16,
        KeyCode::KeyI => 0x17,
        KeyCode::KeyO => 0x18,
        KeyCode::KeyP => 0x19,
        KeyCode::BracketLeft => 0x1A,
        KeyCode::BracketRight => 0x1B,
        KeyCode::Enter => 0x1C,
        KeyCode::ControlLeft => 0x1D,
        KeyCode::KeyA => 0x1E,
        KeyCode::KeyS => 0x1F,
        KeyCode::KeyD => 0x20,
        KeyCode::KeyF => 0x21,
        KeyCode::KeyG => 0x22,
        KeyCode::KeyH => 0x23,
        KeyCode::KeyJ => 0x24,
        KeyCode::KeyK => 0x25,
        KeyCode::KeyL => 0x26,
        KeyCode::Semicolon => 0x27,
        KeyCode::Quote => 0x28,
        KeyCode::Backquote => 0x29,
        KeyCode::ShiftLeft => 0x2A,
        KeyCode::Backslash => 0x2B,
        KeyCode::KeyZ => 0x2C,
        KeyCode::KeyX => 0x2D,
        KeyCode::KeyC => 0x2E,
        KeyCode::KeyV => 0x2F,
        KeyCode::KeyB => 0x30,
        KeyCode::KeyN => 0x31,
        KeyCode::KeyM => 0x32,
        KeyCode::Comma => 0x33,
        KeyCode::Period => 0x34,
        KeyCode::Slash => 0x35,
        KeyCode::ShiftRight => 0x36,
        KeyCode::NumpadMultiply => 0x37,
        KeyCode::AltLeft => 0x38,
        KeyCode::Space => 0x39,
        KeyCode::CapsLock => 0x3A,
        KeyCode::F1 => 0x3B,
        KeyCode::F2 => 0x3C,
        KeyCode::F3 => 0x3D,
        KeyCode::F4 => 0x3E,
        KeyCode::F5 => 0x3F,
        KeyCode::F6 => 0x40,
        KeyCode::F7 => 0x41,
        KeyCode::F8 => 0x42,
        KeyCode::F9 => 0x43,
        KeyCode::F10 => 0x44,
        KeyCode::NumLock => 0x45,
        KeyCode::ScrollLock => 0x46,
        KeyCode::Numpad7 | KeyCode::Home => 0x47,
        KeyCode::Numpad8 | KeyCode::ArrowUp => 0x48,
        KeyCode::Numpad9 | KeyCode::PageUp => 0x49,
        KeyCode::NumpadSubtract => 0x4A,
        KeyCode::Numpad4 | KeyCode::ArrowLeft => 0x4B,
        KeyCode::Numpad5 => 0x4C,
        KeyCode::Numpad6 | KeyCode::ArrowRight => 0x4D,
        KeyCode::NumpadAdd => 0x4E,
        KeyCode::Numpad1 | KeyCode::End => 0x4F,
        KeyCode::Numpad2 | KeyCode::ArrowDown => 0x50,
        KeyCode::Numpad3 | KeyCode::PageDown => 0x51,
        KeyCode::Numpad0 | KeyCode::Insert => 0x52,
        KeyCode::NumpadDecimal | KeyCode::Delete => 0x53,
        KeyCode::F11 => 0x57,
        KeyCode::F12 => 0x58,
        _ => 0x00,
    }
}

fn handle_mouse_button(
    computer: &mut Computer,
    serial_mouse: &Option<Arc<RwLock<SerialMouse>>>,
    ps2_mouse: bool,
    button: MouseButton,
    state: ElementState,
    mouse_left: &mut bool,
    mouse_right: &mut bool,
) {
    let pressed = state == ElementState::Pressed;
    match button {
        MouseButton::Left => *mouse_left = pressed,
        MouseButton::Right => *mouse_right = pressed,
        _ => return,
    }
    if ps2_mouse {
        let buttons = (*mouse_left as u8) | ((*mouse_right as u8) << 1);
        computer.push_ps2_mouse_event(0, 0, buttons);
    } else if let Some(m) = serial_mouse {
        m.write().unwrap().push_buttons(*mouse_left, *mouse_right);
    }
}

fn process_egui_frame(
    egui_ctx: &egui::Context,
    egui_state: &mut egui_winit::State,
    window: &winit::window::Window,
    exclusive_mode: bool,
    app_state: &mut AppState,
    computer: &mut Computer,
    video_buffer: &Arc<RwLock<VideoBuffer>>,
) -> egui::FullOutput {
    // Update menu states before rendering
    app_state.menu.update_debug_states(
        computer.exec_logging_enabled(),
        app_state.is_paused,
        app_state.show_performance_overlay,
    );

    let raw_input = egui_state.take_egui_input(window);
    let full_output = egui_ctx.run(raw_input, |ctx| {
        if !exclusive_mode {
            let action = app_state.menu.render(ctx);

            if let Some(action) = action {
                match action {
                    // TODO
                    // MenuAction::InsertCdRom => {
                    //     todo!();
                    // }
                    // MenuAction::EjectCdRom => {
                    //     todo!();
                    // }
                    _ if action.is_insert() => {
                        show_insert_floppy_dialog(
                            action.drive_number(),
                            computer,
                            &mut app_state.floppy_a_present,
                            &mut app_state.floppy_b_present,
                            &mut app_state.menu,
                            &mut app_state.notification,
                        );
                    }
                    _ if action.is_debug_action() => {
                        handle_debug_action(action, computer, app_state, video_buffer, window);
                    }
                    _ => {
                        eject_floppy_disk(
                            action.drive_number(),
                            computer,
                            &mut app_state.floppy_a_present,
                            &mut app_state.floppy_b_present,
                            &mut app_state.menu,
                            &mut app_state.notification,
                        );
                    }
                }
            }
        }

        // Render performance overlay outside exclusive mode check so it's always visible
        if app_state.show_performance_overlay {
            render_performance_overlay(ctx, app_state.target_mhz, app_state.perf_tracker.get_mhz());
        }

        // Render notification if present and not expired (unless halted)
        if let Some(notification) = &app_state.notification {
            if !app_state.halted && notification.is_expired() {
                app_state.notification = None;
            } else {
                render_notification(ctx, notification);
            }
        }
    });

    egui_state.handle_platform_output(window, full_output.platform_output.clone());
    full_output
}

fn show_insert_floppy_dialog(
    slot: DriveNumber,
    computer: &mut Computer,
    floppy_a_present: &mut bool,
    floppy_b_present: &mut bool,
    menu: &mut AppMenu,
    notification: &mut Option<Notification>,
) {
    let drive_label = if slot == DriveNumber::floppy_a() {
        "A:"
    } else {
        "B:"
    };

    let result = rfd::FileDialog::new()
        .add_filter("Disk Images", &["ima", "img"])
        .set_directory(".")
        .set_title(format!("Select Disk for Floppy {}", drive_label))
        .pick_file();

    if let Some(file) = result {
        let path = file.to_string_lossy().to_string();
        load_and_insert_disk(
            slot,
            &path,
            computer,
            floppy_a_present,
            floppy_b_present,
            menu,
            notification,
        );
    }
}

fn load_and_insert_disk(
    slot: DriveNumber,
    path: &str,
    computer: &mut Computer,
    floppy_a_present: &mut bool,
    floppy_b_present: &mut bool,
    menu: &mut AppMenu,
    notification: &mut Option<Notification>,
) {
    let drive_label = if slot == DriveNumber::floppy_a() {
        "A:"
    } else {
        "B:"
    };

    let result = (|| -> Result<()> {
        let backend = FileDiskBackend::open(path, false)?;
        let disk =
            BackedDisk::new(backend).with_context(|| format!("Invalid disk image: {}", path))?;

        computer.set_floppy_disk(slot, Some(Box::new(disk)));

        log::info!("Inserted floppy {} from {}", slot, path);
        Ok(())
    })();

    match result {
        Ok(()) => {
            if slot == DriveNumber::floppy_a() {
                *floppy_a_present = true;
            } else {
                *floppy_b_present = true;
            }
            menu.update_menu_states(*floppy_a_present, *floppy_b_present);

            let filename = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);
            *notification = Some(Notification::new(
                format!("Loaded {} into drive {}", filename, drive_label),
                NotificationType::Success,
            ));
        }
        Err(e) => {
            log::error!("Failed to insert disk: {:#}", e);

            *notification = Some(Notification::new(
                format!("Failed to load disk into {}: {:#}", drive_label, e),
                NotificationType::Error,
            ));
        }
    }
}

fn eject_floppy_disk(
    slot: DriveNumber,
    computer: &mut Computer,
    floppy_a_present: &mut bool,
    floppy_b_present: &mut bool,
    menu: &mut AppMenu,
    notification: &mut Option<Notification>,
) {
    let drive_label = if slot == DriveNumber::floppy_a() {
        "A:"
    } else {
        "B:"
    };

    match computer.set_floppy_disk(slot, None) {
        Some(_disk) => {
            log::info!("Ejected floppy {}", slot);
            if slot == DriveNumber::floppy_a() {
                *floppy_a_present = false;
            } else {
                *floppy_b_present = false;
            }
            menu.update_menu_states(*floppy_a_present, *floppy_b_present);

            *notification = Some(Notification::new(
                format!("Ejected disk from drive {}", drive_label),
                NotificationType::Success,
            ));
        }
        None => {
            log::warn!("No disk in floppy {} to eject", slot);

            *notification = Some(Notification::new(
                format!("No disk in drive {} to eject", drive_label),
                NotificationType::Error,
            ));
        }
    }
}

fn render_frame(
    pixels: &mut Pixels,
    egui_renderer: &mut egui_wgpu::Renderer,
    egui_ctx: &egui::Context,
    full_output: egui::FullOutput,
    window: &winit::window::Window,
    surface_size: winit::dpi::PhysicalSize<u32>,
) {
    let screen_descriptor = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [surface_size.width, surface_size.height],
        pixels_per_point: window.scale_factor() as f32,
    };

    let clipped_primitives = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

    // Update egui textures
    for (id, image_delta) in &full_output.textures_delta.set {
        egui_renderer.update_texture(pixels.device(), pixels.queue(), *id, image_delta);
    }

    // Render both pixels (emulator) and egui (menu) together
    if let Err(e) = pixels.render_with(|encoder, render_target, context| {
        context.scaling_renderer.render(encoder, render_target);

        egui_renderer.update_buffers(
            pixels.device(),
            pixels.queue(),
            encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        egui_renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);

        Ok(())
    }) {
        log::error!("Failed to render: {}", e);
        std::process::exit(1);
    }

    // Free egui textures
    for id in &full_output.textures_delta.free {
        egui_renderer.free_texture(id);
    }
}

fn step_emulator(
    computer: &mut Computer,
    pixels: &mut Pixels,
    video_buffer: &Arc<RwLock<VideoBuffer>>,
    is_paused: bool,
    throttle: &mut CpuThrottle,
    max_cycles_per_frame: u64,
) -> bool {
    let mut halted = false;

    // Skip execution if paused. When the computer is debug-paused (MCP breakpoint
    // or watchpoint), still call step() once per frame so that debug_check()
    // can service pending MCP commands (step/continue/read_memory).
    if computer.is_debug_paused() {
        computer.step();
    } else if !is_paused {
        let current_cycles = computer.get_cycle_count();
        let frame_target = throttle
            .target_cycles()
            .min(current_cycles + max_cycles_per_frame)
            .max(current_cycles + 1000);

        while computer.get_cycle_count() < frame_target {
            computer.step();
            // Only treat as a terminal halt when IF=0 (e.g. INT 20h / INT 21h AH=4Ch exit).
            // HLT with IF=1 (STI + HLT idle loop used by TSRs/task managers) must keep running
            // so that pending timer IRQs can wake the CPU back up.
            if computer.is_terminal_halt() {
                log::info!("Computer halted");
                halted = true;
                break;
            }
            // If waiting for a keypress, step() returns without advancing cycles,
            // which would spin this loop forever and lock up the window.
            if computer.wait_for_key_press() {
                break;
            }
            // If a debug pause was triggered mid-frame (e.g. by MCP pause command or
            // a breakpoint), step() returns without advancing cycles.  Break out now
            // so the winit event loop stays responsive instead of spinning forever.
            if computer.is_debug_paused() {
                break;
            }
        }
    }

    // Always update the pixel buffer from the video buffer
    let (is_dirty, width, height) = {
        let vb = video_buffer.read().unwrap();
        let (w, h) = vb.mode().resolution();
        (vb.is_dirty(), w, h)
    };
    if is_dirty {
        if pixels.frame_mut().len() != width as usize * height as usize * 4 {
            log::info!("resizing pixel buffer {width}x{height}");
            if let Err(e) = pixels.resize_buffer(width, height) {
                log::error!("Failed to resize pixel buffer to {width}x{height}: {e}");
            }
        }
        video_buffer
            .write()
            .unwrap()
            .render_and_clear_dirty(pixels.frame_mut());
    }

    halted
}

fn handle_debug_action(
    action: menu::MenuAction,
    computer: &mut Computer,
    app_state: &mut AppState,
    video_buffer: &Arc<RwLock<VideoBuffer>>,
    window: &winit::window::Window,
) {
    use menu::MenuAction;

    match action {
        MenuAction::Reset => {
            log::info!("Resetting computer...");
            computer.reset();
            app_state.notification = None;
            app_state.halted = false;
            window.set_title(TITLE);
            log::info!("Computer reset complete");
        }
        MenuAction::SaveScreenshot => {
            save_screenshot(video_buffer, &mut app_state.notification);
        }
        MenuAction::ToggleExecutionLogging => {
            computer.set_exec_logging_enabled(!computer.exec_logging_enabled());
        }
        MenuAction::TogglePause => {
            app_state.is_paused = !app_state.is_paused;
            log::info!(
                "Emulation {}",
                if app_state.is_paused {
                    "paused"
                } else {
                    "resumed"
                }
            );
        }
        MenuAction::TogglePerformanceOverlay => {
            app_state.show_performance_overlay = !app_state.show_performance_overlay;
            log::info!(
                "Performance overlay {}",
                if app_state.show_performance_overlay {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }
        _ => {}
    }
}

fn render_performance_overlay(ctx: &egui::Context, target_mhz: f64, actual_mhz: f64) {
    egui::Window::new("Performance")
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 10.0))
        .title_bar(false)
        .resizable(false)
        .movable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.label(format!("Target: {:.2} MHz", target_mhz));
                ui.label(format!("Actual: {:.2} MHz", actual_mhz));
            });
        });
}

fn render_notification(ctx: &egui::Context, notification: &Notification) {
    egui::Window::new("Notification")
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -50.0))
        .title_bar(false)
        .resizable(false)
        .movable(false)
        .collapsible(false)
        .default_width(500.0)
        .max_width(600.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (icon, color) = match notification.notification_type {
                    NotificationType::Success => ("✅", egui::Color32::from_rgb(0, 200, 0)),
                    NotificationType::Error => ("❌", egui::Color32::from_rgb(220, 0, 0)),
                };

                ui.spacing_mut().item_spacing.x = 8.0;

                ui.label(egui::RichText::new(icon).size(20.0));

                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.set_max_width(520.0);
                    ui.style_mut().wrap = Some(true);
                    ui.label(egui::RichText::new(&notification.message).color(color));
                });
            });
        });
}

fn save_screenshot(
    video_buffer: &Arc<RwLock<VideoBuffer>>,
    notification: &mut Option<Notification>,
) {
    let result = rfd::FileDialog::new()
        .add_filter("PNG Image", &["png"])
        .set_directory(".")
        .set_file_name({
            let now = chrono::Local::now();
            format!(
                "oxide86_screenshot_{:04}{:02}{:02}_{:02}{:02}{:02}.png",
                now.year(),
                now.month(),
                now.day(),
                now.hour(),
                now.minute(),
                now.second()
            )
        })
        .set_title("Save Screenshot")
        .save_file();

    if let Some(file_path) = result {
        let path = file_path.to_string_lossy().to_string();

        let render = {
            let vb = video_buffer.read().unwrap();
            vb.render()
        };

        let save_result = image::save_buffer(
            &path,
            &render.data,
            render.width,
            render.height,
            image::ColorType::Rgba8,
        );

        match save_result {
            Ok(()) => {
                let filename = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&path);
                log::info!("Screenshot saved to {}", path);
                *notification = Some(Notification::new(
                    format!("Screenshot saved as {}", filename),
                    NotificationType::Success,
                ));
            }
            Err(e) => {
                log::error!("Failed to save screenshot: {}", e);
                *notification = Some(Notification::new(
                    format!("Failed to save screenshot: {}", e),
                    NotificationType::Error,
                ));
            }
        }
    }
}
