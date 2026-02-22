mod gui_keyboard;
mod gui_mouse;
mod gui_video;
mod menu;

use anyhow::{Context, Result};
use clap::Parser;
use gui_keyboard::GuiKeyboard;
use gui_mouse::GuiMouse;
use gui_video::{PixelsVideoController, SCREEN_HEIGHT, SCREEN_WIDTH};
use log::LevelFilter;
use menu::AppMenu;
use oxide86_core::NullJoystick;
use oxide86_core::{
    BackedDisk, CGA_MEMORY_END, CGA_MEMORY_SIZE, CGA_MEMORY_START, CdRomImage, Computer,
    DriveNumber, MEMORY_SIZE,
};
use oxide86_native_common::{
    AudioOutput, CommonCli, FileDiskBackend, GilrsJoystick, GilrsJoystickInput, NativeClock,
    apply_logging_flags, attach_serial_device, create_audio, load_cdroms, load_disks,
    load_mounted_directories, load_program_or_boot, sync_mounted_directories,
};
use pixels::{Pixels, SurfaceTexture, wgpu};
use std::fs::File;
use std::time::Instant;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, WindowBuilder};

const TITLE: &str = "Oxide86 - x86 Emulator";

#[derive(Parser)]
#[command(name = "oxide86-gui")]
#[command(about = "Intel 8086 CPU Emulator (GUI)", long_about = None)]
struct Cli {
    #[command(flatten)]
    common: CommonCli,
}

fn main() {
    use std::io::Write;
    let log_file = File::create("oxide86.log").expect("Failed to create log file");

    // Initialize logger from RUST_LOG env var, or use defaults if not set
    let mut builder = env_logger::Builder::from_default_env();

    // Only apply defaults if RUST_LOG is not set
    if std::env::var("RUST_LOG").is_err() {
        builder
            .filter_level(LevelFilter::Error)
            .filter_module("oxide86_core", LevelFilter::Info)
            .filter_module("oxide86_native_gui", LevelFilter::Info);
    }

    // Always filter wgpu logs to reduce noise
    builder
        .filter_module("naga", LevelFilter::Info)
        .filter_module("wgpu_core", LevelFilter::Info)
        .filter_module("wgpu_hal", LevelFilter::Error)
        .filter_module("calloop", LevelFilter::Debug)
        .format(|buf, record| {
            use chrono::Timelike;
            let now = chrono::Local::now();
            writeln!(
                buf,
                "[{:02}:{:02}:{:02}.{:03} {:5} {}] {}",
                now.hour(),
                now.minute(),
                now.second(),
                now.timestamp_subsec_millis(),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        log::error!("Application error: {:#}", e);
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
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

fn attach_serial_devices_from_cli(
    computer: &mut Computer<PixelsVideoController>,
    cli: &Cli,
    gui_mouse: &GuiMouse,
) {
    if let Some(device) = &cli.common.com1_device {
        let mouse_clone = Box::new(gui_mouse.clone_shared()) as Box<dyn oxide86_core::MouseInput>;
        attach_serial_device(computer, 1, device, mouse_clone);
    }
    if let Some(device) = &cli.common.com2_device {
        let mouse_clone = Box::new(gui_mouse.clone_shared()) as Box<dyn oxide86_core::MouseInput>;
        attach_serial_device(computer, 2, device, mouse_clone);
    }
}

/// Wayland + xrdp workaround: detect if MouseMotion reports absolute positions instead of deltas
struct MouseMotionState {
    absolute_mode: bool,
    absolute_mode_detected: bool,
    last_absolute_x: Option<f64>,
    last_absolute_y: Option<f64>,
}

impl MouseMotionState {
    fn new() -> Self {
        Self {
            absolute_mode: false,
            absolute_mode_detected: false,
            last_absolute_x: None,
            last_absolute_y: None,
        }
    }

    fn process_motion(&mut self, delta: (f64, f64)) -> (f64, f64) {
        if !self.absolute_mode_detected {
            // Detection phase: check if these look like absolute positions
            let looks_absolute = (delta.0 > 100.0 && delta.1 > 100.0)
                || (delta.0 > 0.0
                    && delta.1 > 0.0
                    && delta.0 < 10000.0
                    && delta.1 < 10000.0
                    && (delta.0.abs() > 50.0 || delta.1.abs() > 50.0));

            if looks_absolute {
                self.absolute_mode = true;
                self.absolute_mode_detected = true;
                log::warn!(
                    "Detected absolute mouse positioning bug (Wayland+xrdp) - enabling workaround. \
                    First values: ({:.2}, {:.2})",
                    delta.0,
                    delta.1
                );
            } else {
                self.absolute_mode_detected = true;
            }

            if self.absolute_mode {
                self.last_absolute_x = Some(delta.0);
                self.last_absolute_y = Some(delta.1);
                (0.0, 0.0)
            } else {
                delta
            }
        } else if self.absolute_mode {
            let actual_delta = if let (Some(last_x), Some(last_y)) =
                (self.last_absolute_x, self.last_absolute_y)
            {
                (delta.0 - last_x, delta.1 - last_y)
            } else {
                (0.0, 0.0)
            };

            self.last_absolute_x = Some(delta.0);
            self.last_absolute_y = Some(delta.1);
            actual_delta
        } else {
            delta
        }
    }
}

fn handle_window_resize(
    pixels: &mut Pixels,
    computer: &mut Computer<PixelsVideoController>,
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
    // Round to nearest dimensions that maintain the aspect ratio
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
        return; // Don't exit, just log the error and continue
    }

    // Store the actual surface size so render_frame can use it
    *surface_size = adjusted_size;

    computer
        .bios_mut()
        .mouse
        .update_window_size(adjusted_size.width as f64, adjusted_size.height as f64);
}

fn handle_keyboard_input(
    computer: &mut Computer<PixelsVideoController>,
    window: &winit::window::Window,
    input: &winit::event::KeyEvent,
    exclusive_mode: &mut bool,
) -> bool {
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
        return true; // Event handled, skip further processing
    }

    // Convert the event to a KeyPress and fire INT 09h
    if let Some(key) = computer.bios().keyboard.event_to_keypress(input) {
        computer.process_keyboard_irq(key);
    }

    false
}

fn handle_mouse_button(
    computer: &mut Computer<PixelsVideoController>,
    button: MouseButton,
    state: ElementState,
) {
    let button_code = match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        _ => return,
    };
    let pressed = state == ElementState::Pressed;
    computer
        .bios_mut()
        .mouse
        .process_button(button_code, pressed);
}

fn step_emulator(
    computer: &mut Computer<PixelsVideoController>,
    pixels: &mut Pixels,
    is_paused: bool,
    throttle_start: Instant,
    nanos_per_cycle: u64,
) -> bool {
    let mut halted = false;

    // Skip execution if paused
    if !is_paused {
        // Throttled mode: execute cycles to catch up to real time
        // Calculate target cycles based on elapsed wall time
        let elapsed_nanos = throttle_start.elapsed().as_nanos() as u64;
        let target_cycles = elapsed_nanos / nanos_per_cycle;
        let current_cycles = computer.get_cycle_count();

        // Execute until we catch up, but cap per frame to stay responsive.
        // We compare get_cycle_count() (which tracks actual instruction cycles) against the
        // target after every step, so the emulator advances the correct number of CPU cycles
        // regardless of instruction mix (NOPs at 3 cycles, LOOPs at 17, etc.).
        //
        // The cap is ~1.5 frames worth of cycles at the target clock speed so the audio ring
        // buffer stays full regardless of clock speed, while still bounding the per-frame
        // compute time to ≲25 ms (invisible to the user at 60 fps).
        let max_cycles_per_frame = (25_000_000u64 / nanos_per_cycle).max(100_000);
        let frame_target = target_cycles.min(current_cycles + max_cycles_per_frame);

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
        }
    }

    // Always update and render video, even if halted
    // This ensures the final output is visible before showing the termination message
    computer.update_video();

    if computer.video_controller_mut().has_pending_updates() {
        computer.video_controller_mut().render(pixels);
    }

    halted
}

struct PerformanceTracker {
    last_update_time: Instant,
    last_cycle_count: u64,
    current_mhz: f64,
    update_interval_ms: u64,
}

impl PerformanceTracker {
    fn new() -> Self {
        Self {
            last_update_time: Instant::now(),
            last_cycle_count: 0,
            current_mhz: 0.0,
            update_interval_ms: 200,
        }
    }

    fn update(&mut self, current_cycles: u64) {
        let now = Instant::now();
        let elapsed_ms = now.duration_since(self.last_update_time).as_millis() as u64;

        if elapsed_ms >= self.update_interval_ms {
            let cycle_delta = current_cycles.saturating_sub(self.last_cycle_count);
            let time_delta_ms = elapsed_ms as f64;

            // Calculate instantaneous MHz: cycles / milliseconds / 1000
            let instant_mhz = (cycle_delta as f64) / time_delta_ms / 1000.0;

            // Exponential moving average for smoothing
            if self.current_mhz == 0.0 {
                self.current_mhz = instant_mhz;
            } else {
                self.current_mhz = 0.7 * self.current_mhz + 0.3 * instant_mhz;
            }

            self.last_update_time = now;
            self.last_cycle_count = current_cycles;
        }
    }

    fn get_mhz(&self) -> f64 {
        self.current_mhz
    }
}

enum NotificationType {
    Success,
    Error,
}

struct Notification {
    message: String,
    notification_type: NotificationType,
    shown_at: Instant,
}

impl Notification {
    fn new(message: String, notification_type: NotificationType) -> Self {
        Self {
            message,
            notification_type,
            shown_at: Instant::now(),
        }
    }

    fn is_expired(&self) -> bool {
        let duration = match self.notification_type {
            NotificationType::Success => std::time::Duration::from_secs(3),
            NotificationType::Error => std::time::Duration::from_secs(6),
        };
        self.shown_at.elapsed() > duration
    }
}

struct ComputerSetup {
    computer: Computer<PixelsVideoController>,
    gilrs_joystick: Option<GilrsJoystick>,
    mounted_drives: Vec<DriveNumber>,
    audio_output: Option<AudioOutput>,
}

struct AppState {
    menu: AppMenu,
    floppy_a_present: bool,
    floppy_b_present: bool,
    cdrom_present: bool,
    is_paused: bool,
    interrupt_logging_enabled: bool,
    show_performance_overlay: bool,
    perf_tracker: PerformanceTracker,
    notification: Option<Notification>,
    halted: bool,
    target_mhz: f64,
}

fn process_egui_frame(
    egui_ctx: &egui::Context,
    egui_state: &mut egui_winit::State,
    window: &winit::window::Window,
    exclusive_mode: bool,
    app_state: &mut AppState,
    computer: &mut Computer<PixelsVideoController>,
) -> egui::FullOutput {
    // Update menu states before rendering
    app_state.menu.update_debug_states(
        computer.exec_logging_enabled,
        app_state.interrupt_logging_enabled,
        app_state.is_paused,
        app_state.show_performance_overlay,
    );

    let raw_input = egui_state.take_egui_input(window);
    let full_output = egui_ctx.run(raw_input, |ctx| {
        if !exclusive_mode {
            let action = app_state.menu.render(ctx);

            if let Some(action) = action {
                use menu::MenuAction;
                match action {
                    MenuAction::InsertCdRom => {
                        show_insert_cdrom_dialog(
                            computer,
                            &mut app_state.cdrom_present,
                            &mut app_state.menu,
                            &mut app_state.notification,
                        );
                    }
                    MenuAction::EjectCdRom => {
                        eject_cdrom(
                            computer,
                            &mut app_state.cdrom_present,
                            &mut app_state.menu,
                            &mut app_state.notification,
                        );
                    }
                    _ if action.is_insert() => {
                        show_insert_dialog(
                            action.drive_number(),
                            computer,
                            &mut app_state.floppy_a_present,
                            &mut app_state.floppy_b_present,
                            &mut app_state.menu,
                            &mut app_state.notification,
                        );
                    }
                    _ if action.is_debug_action() => {
                        handle_debug_action(action, computer, app_state);
                    }
                    _ => {
                        eject_disk(
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

                // Vertically center the icon and text together
                ui.spacing_mut().item_spacing.x = 8.0;

                // Icon
                ui.label(egui::RichText::new(icon).size(20.0));

                // Message text with wrapping - add top spacing to align with icon center
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    ui.set_max_width(520.0);
                    ui.style_mut().wrap = Some(true);
                    ui.label(egui::RichText::new(&notification.message).color(color));
                });
            });
        });
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

fn run(cli: Cli) -> Result<()> {
    let event_loop = EventLoop::new().context("Failed to create event loop")?;

    let icon = {
        let icon_bytes = include_bytes!("../assets/logo.png");
        image::load_from_memory(icon_bytes)
            .ok()
            .map(|img| img.into_rgba8())
            .and_then(|rgba| {
                let (w, h) = rgba.dimensions();
                winit::window::Icon::from_rgba(rgba.into_raw(), w, h).ok()
            })
    };

    let window = WindowBuilder::new()
        .with_title(TITLE)
        .with_window_icon(icon)
        .with_inner_size(LogicalSize::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32))
        .with_resizable(true)
        .build(&event_loop)
        .context("Failed to create window")?;

    let (window, mut pixels) = create_window_and_pixels(window)?;

    // Create GUI mouse first so we can clone it for serial devices if needed
    // Initialize with actual window size, not logical screen size
    let gui_mouse = GuiMouse::new(
        window.inner_size().width as f64,
        window.inner_size().height as f64,
    );

    // Initialize computer and optional joystick
    let ComputerSetup {
        mut computer,
        mut gilrs_joystick,
        mounted_drives,
        // Keeps the Rodio output stream alive for the duration of emulation.
        audio_output: _audio_output,
    } = create_computer(&cli, gui_mouse.clone_shared())?;

    // Apply logging flags
    apply_logging_flags(&mut computer, cli.common.exec_log, cli.common.int_log);

    // Attach serial devices if specified
    attach_serial_devices_from_cli(&mut computer, &cli, &gui_mouse);

    // Initialize egui
    let (egui_ctx, mut egui_state, mut egui_renderer) = setup_egui(window, &pixels);

    // Create application state
    let mut app_state = AppState {
        menu: AppMenu::new(),
        floppy_a_present: cli.common.floppy_a.is_some(),
        floppy_b_present: cli.common.floppy_b.is_some(),
        cdrom_present: !cli.common.cdroms.is_empty(),
        is_paused: false,
        interrupt_logging_enabled: cli.common.int_log,
        show_performance_overlay: false,
        perf_tracker: PerformanceTracker::new(),
        notification: None,
        halted: false,
        target_mhz: cli.common.speed,
    };
    app_state.menu.update_cdrom_state(app_state.cdrom_present);

    // Speed throttling state
    let cpu_freq = (cli.common.speed * 1_000_000.0) as u64;
    let nanos_per_cycle = 1_000_000_000u64 / cpu_freq;
    let throttle_start = Instant::now();

    log::info!("Running at {:.2} MHz ({} Hz)", cli.common.speed, cpu_freq);

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

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            // Handle device events for raw mouse input (only when cursor is truly locked)
            if let Event::DeviceEvent { event, .. } = &event
                && let DeviceEvent::MouseMotion { delta } = event
                && exclusive_mode
            {
                let (actual_delta_x, actual_delta_y) = mouse_motion_state.process_motion(*delta);

                computer
                    .bios_mut()
                    .mouse
                    .process_relative_motion(actual_delta_x, actual_delta_y);
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
                    // If egui consumed the event (e.g., clicking on menu), don't process it further
                    if response.consumed {
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
                        log::warn!(
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
                        // Sync mounted directories before exit
                        if let Err(e) = sync_mounted_directories(&mut computer, &mounted_drives) {
                            log::error!("Failed to sync mounted directories: {}", e);
                        }
                        std::process::exit(0);
                    }
                    WindowEvent::Resized(new_size) => {
                        handle_window_resize(
                            &mut pixels,
                            &mut computer,
                            new_size,
                            &mut surface_size,
                        );
                    }
                    WindowEvent::KeyboardInput { event: input, .. } => {
                        // Always handle keyboard input (F12 to exit exclusive mode, etc.)
                        // Even when halted, F12 should still unlock the cursor
                        handle_keyboard_input(&mut computer, window, &input, &mut exclusive_mode);
                    }
                    WindowEvent::ModifiersChanged(modifiers) => {
                        let mods = modifiers.state();
                        computer.bios_mut().keyboard.update_modifiers(&mods);

                        // Update BDA keyboard flags so INT 16h AH=02h works correctly
                        computer.update_keyboard_flags(
                            modifiers.state().shift_key(),
                            modifiers.state().control_key(),
                            modifiers.state().alt_key(),
                        );
                    }
                    WindowEvent::MouseInput { button, state, .. } => {
                        if exclusive_mode {
                            handle_mouse_button(&mut computer, button, state);
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        // Poll joystick for gamepad events if enabled
                        if let Some(ref mut js) = gilrs_joystick {
                            js.poll();
                        }

                        let halted = step_emulator(
                            &mut computer,
                            &mut pixels,
                            app_state.is_paused,
                            throttle_start,
                            nanos_per_cycle,
                        );

                        // Handle halt: show notification
                        if halted && !app_state.halted {
                            app_state.halted = true;
                            app_state.notification = Some(Notification::new(
                                "Program terminated. Close window to exit.".to_string(),
                                NotificationType::Success,
                            ));
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

fn create_computer(cli: &Cli, gui_mouse: GuiMouse) -> Result<ComputerSetup> {
    // Parse CPU type
    let cpu_type = oxide86_core::CpuType::parse(&cli.common.cpu_type)
        .ok_or_else(|| anyhow::anyhow!("Invalid CPU type: {}", cli.common.cpu_type))?;

    // Parse video card type
    let video_card_type = oxide86_core::VideoCardType::parse(&cli.common.video_card)
        .ok_or_else(|| anyhow::anyhow!("Invalid video card type: {}", cli.common.video_card))?;

    // Create computer with keyboard, mouse, joystick, video, and speaker
    let keyboard = Box::new(GuiKeyboard::new());
    let mouse = Box::new(gui_mouse);

    // Create joystick - only initialize gilrs if joystick flags are set
    let (joystick, gilrs_joystick): (Box<dyn oxide86_core::JoystickInput>, Option<GilrsJoystick>) =
        if cli.common.joystick_a || cli.common.joystick_b {
            log::info!("Initializing gamepad support (gilrs)");
            let gilrs = GilrsJoystick::new();
            let input = Box::new(GilrsJoystickInput::new(gilrs.clone_state()));
            (input, Some(gilrs))
        } else {
            (Box::new(NullJoystick), None)
        };

    let video = PixelsVideoController::new();
    let cpu_freq = (cli.common.speed * 1_000_000.0) as u64;
    let (speaker, sound_card, audio_output) = create_audio(
        !cli.common.disable_pc_speaker,
        &cli.common.sound_card,
        cpu_freq,
    );

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

    // Force initial video render to show blank screen
    computer.force_video_redraw();

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

    log::info!("Starting execution...");

    Ok(ComputerSetup {
        computer,
        gilrs_joystick,
        mounted_drives,
        audio_output,
    })
}

fn show_insert_dialog(
    slot: DriveNumber,
    computer: &mut Computer<PixelsVideoController>,
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
    computer: &mut Computer<PixelsVideoController>,
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

        computer
            .bios_mut()
            .insert_floppy(slot, Box::new(disk))
            .map_err(|e| anyhow::anyhow!(e))?;

        log::info!("Inserted floppy {} from {}", slot, path);
        Ok(())
    })();

    match result {
        Ok(()) => {
            // Update state
            if slot == DriveNumber::floppy_a() {
                *floppy_a_present = true;
            } else {
                *floppy_b_present = true;
            }
            // Update menu
            menu.update_menu_states(*floppy_a_present, *floppy_b_present);

            // Show success notification
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

            // Show error notification
            *notification = Some(Notification::new(
                format!("Failed to load disk into {}: {:#}", drive_label, e),
                NotificationType::Error,
            ));
        }
    }
}

fn eject_disk(
    slot: DriveNumber,
    computer: &mut Computer<PixelsVideoController>,
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

    match computer.bios_mut().eject_floppy(slot) {
        Ok(Some(_disk)) => {
            log::info!("Ejected floppy {}", slot);
            // Update state
            if slot == DriveNumber::floppy_a() {
                *floppy_a_present = false;
            } else {
                *floppy_b_present = false;
            }
            // Update menu
            menu.update_menu_states(*floppy_a_present, *floppy_b_present);

            // Show success notification
            *notification = Some(Notification::new(
                format!("Ejected disk from drive {}", drive_label),
                NotificationType::Success,
            ));
        }
        Ok(None) => {
            log::warn!("No disk in floppy {} to eject", slot);

            // Show warning notification
            *notification = Some(Notification::new(
                format!("No disk in drive {} to eject", drive_label),
                NotificationType::Error,
            ));
        }
        Err(e) => {
            log::error!("Failed to eject disk: {}", e);

            // Show error notification
            *notification = Some(Notification::new(
                format!("Failed to eject disk from {}: {}", drive_label, e),
                NotificationType::Error,
            ));
        }
    }
}

fn show_insert_cdrom_dialog(
    computer: &mut Computer<PixelsVideoController>,
    cdrom_present: &mut bool,
    menu: &mut AppMenu,
    notification: &mut Option<Notification>,
) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("ISO Images", &["iso"])
        .pick_file()
    {
        match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(CdRomImage::new)
        {
            Ok(image) => {
                computer.bios_mut().insert_cdrom(0, image);
                *cdrom_present = true;
                menu.update_cdrom_state(true);
                let label = path.file_name().unwrap_or_default().to_string_lossy();
                *notification = Some(Notification::new(
                    format!("Inserted CD-ROM: {}", label),
                    NotificationType::Success,
                ));
                log::info!("Inserted CD-ROM from {}", path.display());
            }
            Err(e) => {
                *notification = Some(Notification::new(
                    format!("Failed to load ISO: {}", e),
                    NotificationType::Error,
                ));
                log::error!("Failed to load CD-ROM image: {}", e);
            }
        }
    }
}

fn eject_cdrom(
    computer: &mut Computer<PixelsVideoController>,
    cdrom_present: &mut bool,
    menu: &mut AppMenu,
    notification: &mut Option<Notification>,
) {
    match computer.bios_mut().eject_cdrom(0) {
        Some(_) => {
            *cdrom_present = false;
            menu.update_cdrom_state(false);
            *notification = Some(Notification::new(
                "CD-ROM ejected".to_string(),
                NotificationType::Success,
            ));
            log::info!("Ejected CD-ROM");
        }
        None => {
            *notification = Some(Notification::new(
                "No CD-ROM to eject".to_string(),
                NotificationType::Error,
            ));
        }
    }
}

fn save_screenshot(
    computer: &Computer<PixelsVideoController>,
    notification: &mut Option<Notification>,
) {
    use chrono::{Datelike, Timelike};

    // Show file dialog
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

        // Get the frame buffer from the video controller
        let buffer = computer.video_controller().render_to_buffer();

        // Save as PNG using the image crate
        let save_result = image::save_buffer(
            &path,
            &buffer,
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32,
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

fn save_ram(computer: &Computer<PixelsVideoController>, notification: &mut Option<Notification>) {
    use chrono::{Datelike, Timelike};

    // Show file dialog
    let result = rfd::FileDialog::new()
        .add_filter("Binary File", &["bin"])
        .set_directory(".")
        .set_file_name({
            let now = chrono::Local::now();
            format!(
                "oxide86_ram_{:04}{:02}{:02}_{:02}{:02}{:02}.bin",
                now.year(),
                now.month(),
                now.day(),
                now.hour(),
                now.minute(),
                now.second()
            )
        })
        .set_title("Save RAM")
        .save_file();

    if let Some(file_path) = result {
        let path = file_path.to_string_lossy().to_string();

        // Get the memory data
        let memory_data = computer.memory().data();

        // Write to file
        let save_result = std::fs::write(&path, memory_data);

        match save_result {
            Ok(()) => {
                let filename = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&path);
                log::info!("RAM saved to {} ({} bytes)", path, MEMORY_SIZE);
                *notification = Some(Notification::new(
                    format!("RAM saved as {} ({} bytes)", filename, MEMORY_SIZE),
                    NotificationType::Success,
                ));
            }
            Err(e) => {
                log::error!("Failed to save RAM: {}", e);
                *notification = Some(Notification::new(
                    format!("Failed to save RAM: {}", e),
                    NotificationType::Error,
                ));
            }
        }
    }
}

fn save_video_ram(
    computer: &Computer<PixelsVideoController>,
    notification: &mut Option<Notification>,
) {
    use chrono::{Datelike, Timelike};

    // Show file dialog
    let result = rfd::FileDialog::new()
        .add_filter("Binary File", &["bin"])
        .set_directory(".")
        .set_file_name({
            let now = chrono::Local::now();
            format!(
                "oxide86_vram_{:04}{:02}{:02}_{:02}{:02}{:02}.bin",
                now.year(),
                now.month(),
                now.day(),
                now.hour(),
                now.minute(),
                now.second()
            )
        })
        .set_title("Save Video RAM")
        .save_file();

    if let Some(file_path) = result {
        let path = file_path.to_string_lossy().to_string();

        // Get the memory data and extract video RAM portion
        let memory_data = computer.memory().data();
        let video_ram = &memory_data[CGA_MEMORY_START..=CGA_MEMORY_END];

        // Write to file
        let save_result = std::fs::write(&path, video_ram);

        match save_result {
            Ok(()) => {
                let filename = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&path);
                log::info!("Video RAM saved to {} ({} bytes)", path, CGA_MEMORY_SIZE);
                *notification = Some(Notification::new(
                    format!(
                        "Video RAM saved as {} ({} bytes)",
                        filename, CGA_MEMORY_SIZE
                    ),
                    NotificationType::Success,
                ));
            }
            Err(e) => {
                log::error!("Failed to save video RAM: {}", e);
                *notification = Some(Notification::new(
                    format!("Failed to save video RAM: {}", e),
                    NotificationType::Error,
                ));
            }
        }
    }
}

fn handle_debug_action(
    action: menu::MenuAction,
    computer: &mut Computer<PixelsVideoController>,
    app_state: &mut AppState,
) {
    use menu::MenuAction;

    match action {
        MenuAction::Reset => {
            log::info!("Resetting computer...");
            computer.reset();
            app_state.notification = None;
            app_state.halted = false;
            log::info!("Computer reset complete");
        }
        MenuAction::SaveScreenshot => {
            save_screenshot(computer, &mut app_state.notification);
        }
        MenuAction::SaveRam => {
            save_ram(computer, &mut app_state.notification);
        }
        MenuAction::SaveVideoRam => {
            save_video_ram(computer, &mut app_state.notification);
        }
        MenuAction::ToggleExecutionLogging => {
            computer.set_exec_logging(!computer.exec_logging_enabled);
        }
        MenuAction::ToggleInterruptLogging => {
            app_state.interrupt_logging_enabled = !app_state.interrupt_logging_enabled;
            computer.set_log_interrupts(app_state.interrupt_logging_enabled);
            log::info!(
                "Interrupt logging {}",
                if app_state.interrupt_logging_enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            );
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
