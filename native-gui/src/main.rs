mod gui_keyboard;
mod gui_mouse;
mod gui_video;
mod menu;

use anyhow::{Context, Result};
use clap::Parser;
use emu86_core::utils::parse_hex_or_dec;
use emu86_core::{
    BackedDisk, Computer, DiskController, DriveNumber, FileDiskBackend, NullSpeaker,
    PartitionedDisk, RodioSpeaker, parse_mbr,
};
use gui_keyboard::GuiKeyboard;
use gui_mouse::GuiMouse;
use gui_video::{PixelsVideoController, SCREEN_HEIGHT, SCREEN_WIDTH};
use log::LevelFilter;
use menu::AppMenu;
use pixels::{Pixels, SurfaceTexture, wgpu};
use std::fs::File;
use std::time::Instant;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, WindowBuilder};

const TITLE: &str = "emu86 - 8086 Emulator";

#[derive(Parser)]
#[command(name = "emu86-gui")]
#[command(about = "Intel 8086 CPU Emulator (GUI)", long_about = None)]
struct Cli {
    /// Boot from disk image
    #[arg(long)]
    boot: bool,

    /// Boot drive number (0x00 for floppy A:, 0x01 for floppy B:, 0x80 for hard disk C:)
    #[arg(long, default_value = "0x00")]
    boot_drive: String,

    /// Path to disk image file for floppy A:
    #[arg(long = "floppy-a")]
    floppy_a: Option<String>,

    /// Path to disk image file for floppy B:
    #[arg(long = "floppy-b")]
    floppy_b: Option<String>,

    /// Path to hard disk image file(s) - can be specified multiple times for C:, D:, etc.
    #[arg(long = "hdd", action = clap::ArgAction::Append)]
    hard_disks: Vec<String>,

    /// Device to attach to COM1 (e.g., "mouse")
    #[arg(long = "com1", value_name = "DEVICE")]
    com1_device: Option<String>,

    /// Device to attach to COM2 (e.g., "mouse")
    #[arg(long = "com2", value_name = "DEVICE")]
    com2_device: Option<String>,

    /// Run at maximum speed (no throttling)
    #[arg(long)]
    turbo: bool,
}

fn main() {
    let log_file = File::create("emu86.log").expect("Failed to create log file");
    env_logger::Builder::from_default_env()
        .filter_module("wgpu_core", LevelFilter::Info)
        .filter_module("wgpu_hal", LevelFilter::Info)
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        log::error!("Application error: {}", e);
        eprintln!("Error: {}", e);
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

fn attach_serial_devices(
    computer: &mut Computer<PixelsVideoController>,
    cli: &Cli,
    gui_mouse: &GuiMouse,
) {
    if let Some(device) = &cli.com1_device {
        match device.as_str() {
            "mouse" => {
                use emu86_core::SerialMouse;
                let mouse_clone =
                    Box::new(gui_mouse.clone_shared()) as Box<dyn emu86_core::MouseInput>;
                computer.set_com1_device(Box::new(SerialMouse::new(mouse_clone)));
                log::info!("Serial mouse attached to COM1");
            }
            _ => {
                eprintln!("Warning: Unknown device '{}' for COM1", device);
            }
        }
    }

    if let Some(device) = &cli.com2_device {
        match device.as_str() {
            "mouse" => {
                use emu86_core::SerialMouse;
                let mouse_clone =
                    Box::new(gui_mouse.clone_shared()) as Box<dyn emu86_core::MouseInput>;
                computer.set_com2_device(Box::new(SerialMouse::new(mouse_clone)));
                log::info!("Serial mouse attached to COM2");
            }
            _ => {
                eprintln!("Warning: Unknown device '{}' for COM2", device);
            }
        }
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
) {
    if let Err(e) = pixels.resize_surface(new_size.width, new_size.height) {
        log::error!("Failed to resize surface: {}", e);
        std::process::exit(1);
    }
    computer
        .bios_mut()
        .mouse
        .update_window_size(new_size.width as f64, new_size.height as f64);
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
    turbo_mode: bool,
    throttle_start: Instant,
    nanos_per_cycle: u64,
) {
    // Skip execution if paused
    if !is_paused {
        if turbo_mode {
            // Turbo mode: execute a large batch per frame
            const BATCH_SIZE: u32 = 50000;
            for _ in 0..BATCH_SIZE {
                if computer.is_halted() {
                    log::info!("Computer halted");
                    std::process::exit(0);
                }
                computer.step();
            }
        } else {
            // Throttled mode: execute cycles to catch up to real time
            // Calculate target cycles based on elapsed wall time
            let elapsed_nanos = throttle_start.elapsed().as_nanos() as u64;
            let target_cycles = elapsed_nanos / nanos_per_cycle;
            let current_cycles = computer.get_cycle_count();

            // Execute until we catch up, but cap per frame to stay responsive
            const MAX_CYCLES_PER_FRAME: u64 = 100_000;
            let cycles_to_run =
                (target_cycles.saturating_sub(current_cycles)).min(MAX_CYCLES_PER_FRAME);

            // Each step is ~10 cycles
            let steps_to_run = cycles_to_run / 10;
            for _ in 0..steps_to_run {
                if computer.is_halted() {
                    log::info!("Computer halted");
                    std::process::exit(0);
                }
                computer.step();
            }
        }
    }

    computer.update_video();

    if computer.video_controller_mut().has_pending_updates() {
        computer.video_controller_mut().render(pixels);
    }
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

struct AppState {
    menu: AppMenu,
    floppy_a_present: bool,
    floppy_b_present: bool,
    is_paused: bool,
    interrupt_logging_enabled: bool,
    turbo_mode: bool,
    show_performance_overlay: bool,
    perf_tracker: PerformanceTracker,
    notification: Option<Notification>,
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
        app_state.turbo_mode,
        app_state.show_performance_overlay,
    );

    let raw_input = egui_state.take_egui_input(window);
    let full_output = egui_ctx.run(raw_input, |ctx| {
        if !exclusive_mode {
            let action = app_state.menu.render(ctx);

            if let Some(action) = action {
                if action.is_insert() {
                    show_insert_dialog(
                        action.drive_number(),
                        computer,
                        &mut app_state.floppy_a_present,
                        &mut app_state.floppy_b_present,
                        &mut app_state.menu,
                        &mut app_state.notification,
                    );
                } else if action.is_debug_action() {
                    handle_debug_action(
                        action,
                        computer,
                        &mut app_state.is_paused,
                        &mut app_state.interrupt_logging_enabled,
                        &mut app_state.turbo_mode,
                        &mut app_state.show_performance_overlay,
                    );
                } else {
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

        // Render performance overlay outside exclusive mode check so it's always visible
        if app_state.show_performance_overlay {
            render_performance_overlay(ctx, app_state.turbo_mode, app_state.perf_tracker.get_mhz());
        }

        // Render notification if present and not expired
        if let Some(notification) = &app_state.notification {
            if notification.is_expired() {
                app_state.notification = None;
            } else {
                render_notification(ctx, notification);
            }
        }
    });

    egui_state.handle_platform_output(window, full_output.platform_output.clone());
    full_output
}

fn render_performance_overlay(ctx: &egui::Context, turbo_mode: bool, actual_mhz: f64) {
    egui::Window::new("Performance")
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 10.0))
        .title_bar(false)
        .resizable(false)
        .movable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                let target_text = if turbo_mode {
                    "Target: Unlimited".to_string()
                } else {
                    "Target: 4.77 MHz".to_string()
                };
                ui.label(target_text);
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
                    NotificationType::Success => ("✓", egui::Color32::from_rgb(0, 200, 0)),
                    NotificationType::Error => ("✗", egui::Color32::from_rgb(220, 0, 0)),
                };
                ui.label(egui::RichText::new(icon).color(color).size(20.0));
                ui.vertical(|ui| {
                    ui.set_max_width(550.0);
                    ui.style_mut().wrap = Some(true);
                    ui.label(&notification.message);
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
) {
    let screen_descriptor = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [window.inner_size().width, window.inner_size().height],
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

    let window = WindowBuilder::new()
        .with_title(TITLE)
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

    // Initialize computer
    let mut computer = create_computer(&cli, gui_mouse.clone_shared())?;

    // Attach serial devices if specified
    attach_serial_devices(&mut computer, &cli, &gui_mouse);

    // Initialize egui
    let (egui_ctx, mut egui_state, mut egui_renderer) = setup_egui(window, &pixels);

    // Create application state
    let mut app_state = AppState {
        menu: AppMenu::new(),
        floppy_a_present: cli.floppy_a.is_some(),
        floppy_b_present: cli.floppy_b.is_some(),
        is_paused: false,
        interrupt_logging_enabled: false,
        turbo_mode: cli.turbo,
        show_performance_overlay: false,
        perf_tracker: PerformanceTracker::new(),
        notification: None,
    };

    // Speed throttling state (4.77 MHz = original 8086 speed)
    const CLOCK_HZ: u64 = 4_770_000;
    const NANOS_PER_CYCLE: u64 = 1_000_000_000 / CLOCK_HZ;
    let throttle_start = Instant::now();

    if cli.turbo {
        log::info!("Running in turbo mode (no speed limit)");
    } else {
        log::info!("Running at 4.77 MHz");
    }

    // Update menu states
    app_state
        .menu
        .update_menu_states(app_state.floppy_a_present, app_state.floppy_b_present);

    // Exclusive mode state - when true, hides cursor and disables menu
    let mut exclusive_mode = false;
    // Track cursor position to detect clicks in menu area
    let mut cursor_y = 0.0;

    let mut mouse_motion_state = MouseMotionState::new();

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
                        std::process::exit(0);
                    }
                    WindowEvent::Resized(new_size) => {
                        handle_window_resize(&mut pixels, &mut computer, new_size);
                    }
                    WindowEvent::KeyboardInput { event: input, .. } => {
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
                        step_emulator(
                            &mut computer,
                            &mut pixels,
                            app_state.is_paused,
                            app_state.turbo_mode,
                            throttle_start,
                            NANOS_PER_CYCLE,
                        );

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
                        );

                        window.request_redraw();
                    }
                    _ => {}
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("Event loop error: {}", e))
}

fn create_computer(cli: &Cli, gui_mouse: GuiMouse) -> Result<Computer<PixelsVideoController>> {
    // Create computer with keyboard, mouse, video, and speaker
    let keyboard = Box::new(GuiKeyboard::new());
    let mouse = Box::new(gui_mouse);
    let video = PixelsVideoController::new();

    // Try to create speaker with fallback
    let speaker: Box<dyn emu86_core::SpeakerOutput> = match RodioSpeaker::new() {
        Ok(rodio_speaker) => {
            log::info!("PC speaker enabled (Rodio)");
            Box::new(rodio_speaker)
        }
        Err(e) => {
            log::warn!("PC speaker unavailable: {}", e);
            log::info!("Using NullSpeaker (no audio)");
            Box::new(NullSpeaker)
        }
    };

    let mut computer = Computer::new(keyboard, mouse, video, speaker);

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
        log::info!("Starting execution...\n");
    }

    Ok(computer)
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
        .add_filter("Disk Images", &["img"])
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
            log::error!("Failed to insert disk: {}", e);

            // Show error notification
            *notification = Some(Notification::new(
                format!("Failed to load disk into {}: {}", drive_label, e),
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

fn handle_debug_action(
    action: menu::MenuAction,
    computer: &mut Computer<PixelsVideoController>,
    is_paused: &mut bool,
    interrupt_logging_enabled: &mut bool,
    turbo_mode: &mut bool,
    show_performance_overlay: &mut bool,
) {
    use menu::MenuAction;

    match action {
        MenuAction::Reset => {
            log::info!("Resetting computer...");
            computer.reset();
            log::info!("Computer reset complete");
        }
        MenuAction::ToggleExecutionLogging => {
            computer.exec_logging_enabled = !computer.exec_logging_enabled;
            log::info!(
                "Execution logging {}",
                if computer.exec_logging_enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }
        MenuAction::ToggleInterruptLogging => {
            *interrupt_logging_enabled = !*interrupt_logging_enabled;
            computer.set_log_interrupts(*interrupt_logging_enabled);
            log::info!(
                "Interrupt logging {}",
                if *interrupt_logging_enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }
        MenuAction::TogglePause => {
            *is_paused = !*is_paused;
            log::info!(
                "Emulation {}",
                if *is_paused { "paused" } else { "resumed" }
            );
        }
        MenuAction::ToggleTurbo => {
            *turbo_mode = !*turbo_mode;
            log::info!(
                "Turbo mode {}",
                if *turbo_mode { "enabled" } else { "disabled" }
            );
        }
        MenuAction::TogglePerformanceOverlay => {
            *show_performance_overlay = !*show_performance_overlay;
            log::info!(
                "Performance overlay {}",
                if *show_performance_overlay {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }
        _ => {}
    }
}
