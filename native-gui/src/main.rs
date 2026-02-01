mod font;
mod gui_keyboard;
mod gui_mouse;
mod gui_video;
mod menu;

use anyhow::{Context, Result};
use clap::Parser;
use emu86_core::utils::parse_hex_or_dec;
use emu86_core::{
    BackedDisk, Computer, DiskController, DriveNumber, FileDiskBackend, PartitionedDisk, parse_mbr,
};
use gui_keyboard::GuiKeyboard;
use gui_mouse::GuiMouse;
use gui_video::{PixelsVideoController, SCREEN_HEIGHT, SCREEN_WIDTH};
use log::LevelFilter;
use menu::AppMenu;
use pixels::{Pixels, SurfaceTexture, wgpu};
use std::fs::File;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, WindowBuilder};

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

fn run(cli: Cli) -> Result<()> {
    let event_loop = EventLoop::new().context("Failed to create event loop")?;

    let window = WindowBuilder::new()
        .with_title("emu86 - 8086 Emulator")
        .with_inner_size(LogicalSize::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32))
        .with_resizable(true)
        .build(&event_loop)
        .context("Failed to create window")?;

    // Leak window to get a 'static reference for the event loop
    let window: &'static _ = Box::leak(Box::new(window));
    let window_size = window.inner_size();

    // Create pixels surface
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, window);
    let mut pixels = Pixels::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface_texture)
        .context("Failed to create Pixels")?;

    // Create GUI mouse first so we can clone it for serial devices if needed
    // Initialize with actual window size, not logical screen size
    let gui_mouse = GuiMouse::new(window_size.width as f64, window_size.height as f64);

    // Initialize computer
    let mut computer = create_computer(&cli, gui_mouse.clone_shared())?;

    // Attach serial devices if specified
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

    // Initialize egui
    let egui_ctx = egui::Context::default();
    let mut egui_state = egui_winit::State::new(
        egui_ctx.clone(),
        egui::ViewportId::ROOT,
        &window,
        None,
        None,
    );

    // Create egui renderer using the same wgpu device as pixels
    let device = pixels.device();
    let target_format = pixels.render_texture_format();

    let mut egui_renderer = egui_wgpu::Renderer::new(device, target_format, None, 1);

    // Create menu
    let mut menu = AppMenu::new();

    // Check initial disk presence from CLI args
    let mut floppy_a_present = cli.floppy_a.is_some();
    let mut floppy_b_present = cli.floppy_b.is_some();

    // Update menu states
    menu.update_menu_states(floppy_a_present, floppy_b_present);

    // Exclusive mode state - when true, hides cursor and disables menu
    let mut exclusive_mode = false;
    // Track cursor position to detect clicks in menu area
    let mut cursor_y = 0.0;

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            // Handle device events for raw mouse input (only when cursor is truly locked)
            if let Event::DeviceEvent { event, .. } = &event {
                if let DeviceEvent::MouseMotion { delta } = event {
                    if exclusive_mode {
                        log::debug!(
                            "DeviceEvent::MouseMotion - delta=({:.2}, {:.2})",
                            delta.0,
                            delta.1
                        );
                        computer
                            .bios_mut()
                            .mouse
                            .process_relative_motion(delta.0, delta.1);
                    }
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
                        if let Err(e) = pixels.resize_surface(new_size.width, new_size.height) {
                            log::error!("Failed to resize surface: {}", e);
                            std::process::exit(1);
                        }
                        // Update mouse coordinate scaling for new window size
                        computer
                            .bios_mut()
                            .mouse
                            .update_window_size(new_size.width as f64, new_size.height as f64);
                    }
                    WindowEvent::KeyboardInput { event: input, .. } => {
                        // Check if F12 is pressed to exit exclusive mode
                        if input.state == ElementState::Pressed
                            && let PhysicalKey::Code(KeyCode::F12) = input.physical_key
                            && exclusive_mode
                        {
                            // Exit exclusive mode
                            exclusive_mode = false;
                            window.set_cursor_visible(true);

                            // Release cursor grab
                            if let Err(e) = window.set_cursor_grab(CursorGrabMode::None) {
                                log::warn!("Failed to release cursor grab: {}", e);
                            }

                            log::info!("Exited exclusive mode (F12)");
                            return;
                        }

                        // Convert the event to a KeyPress and fire INT 09h
                        if let Some(key) = computer.bios().keyboard.event_to_keypress(&input) {
                            computer.process_keyboard_irq(key);
                        }
                    }
                    WindowEvent::ModifiersChanged(modifiers) => {
                        computer
                            .bios_mut()
                            .keyboard
                            .update_modifiers(modifiers.state());

                        // Update BDA keyboard flags so INT 16h AH=02h works correctly
                        computer.update_keyboard_flags(
                            modifiers.state().shift_key(),
                            modifiers.state().control_key(),
                            modifiers.state().alt_key(),
                        );
                    }
                    WindowEvent::MouseInput { button, state, .. } => {
                        // Only process mouse buttons when in exclusive mode
                        // (The first click enters exclusive mode, handled earlier)
                        if exclusive_mode {
                            // Convert winit button to mouse button code
                            let button_code = match button {
                                MouseButton::Left => 0,
                                MouseButton::Right => 1,
                                MouseButton::Middle => 2,
                                _ => return, // Ignore other buttons
                            };
                            let pressed = state == ElementState::Pressed;
                            computer
                                .bios_mut()
                                .mouse
                                .process_button(button_code, pressed);
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        const BATCH_SIZE: u32 = 10000;

                        for _ in 0..BATCH_SIZE {
                            if computer.is_halted() {
                                log::info!("Computer halted");
                                std::process::exit(0);
                            }
                            computer.step();
                        }

                        // Update video from emulator state (only updates if video is dirty)
                        computer.update_video();

                        // Render emulator screen
                        if computer.video_controller_mut().has_pending_updates() {
                            computer.video_controller_mut().render(&mut pixels);
                        }

                        // Render egui (menu bar overlays emulator when not in exclusive mode)
                        // Skip menu rendering when in exclusive mode
                        let raw_input = egui_state.take_egui_input(window);
                        let full_output = egui_ctx.run(raw_input, |ctx| {
                            if !exclusive_mode {
                                let action = menu.render(ctx);

                                if let Some(action) = action {
                                    if action.is_insert() {
                                        show_insert_dialog(
                                            action.drive_number(),
                                            &mut computer,
                                            &mut floppy_a_present,
                                            &mut floppy_b_present,
                                            &mut menu,
                                        );
                                    } else {
                                        eject_disk(
                                            action.drive_number(),
                                            &mut computer,
                                            &mut floppy_a_present,
                                            &mut floppy_b_present,
                                            &mut menu,
                                        );
                                    }
                                }
                            }
                        });

                        egui_state.handle_platform_output(window, full_output.platform_output);

                        // Prepare egui rendering
                        let screen_descriptor = egui_wgpu::ScreenDescriptor {
                            size_in_pixels: [window.inner_size().width, window.inner_size().height],
                            pixels_per_point: window.scale_factor() as f32,
                        };

                        let clipped_primitives =
                            egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

                        // Update egui textures
                        for (id, image_delta) in &full_output.textures_delta.set {
                            egui_renderer.update_texture(
                                pixels.device(),
                                pixels.queue(),
                                *id,
                                image_delta,
                            );
                        }

                        // Render both pixels (emulator) and egui (menu) together
                        if let Err(e) = pixels.render_with(|encoder, render_target, context| {
                            // Render the emulator screen
                            context.scaling_renderer.render(encoder, render_target);

                            // Prepare egui buffers
                            egui_renderer.update_buffers(
                                pixels.device(),
                                pixels.queue(),
                                encoder,
                                &clipped_primitives,
                                &screen_descriptor,
                            );

                            // Then render egui on top
                            let mut render_pass =
                                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

                            egui_renderer.render(
                                &mut render_pass,
                                &clipped_primitives,
                                &screen_descriptor,
                            );

                            Ok(())
                        }) {
                            log::error!("Failed to render: {}", e);
                            std::process::exit(1);
                        }

                        // Free egui textures
                        for id in &full_output.textures_delta.free {
                            egui_renderer.free_texture(id);
                        }

                        // Request another redraw
                        window.request_redraw();
                    }
                    _ => {}
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("Event loop error: {}", e))
}

fn create_computer(
    cli: &Cli,
    gui_mouse: GuiMouse,
) -> Result<Computer<GuiKeyboard, PixelsVideoController>> {
    // Create computer with keyboard and mouse
    let keyboard = GuiKeyboard::new();
    let mouse = Box::new(gui_mouse);
    let video = PixelsVideoController::new();
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
    computer: &mut Computer<GuiKeyboard, PixelsVideoController>,
    floppy_a_present: &mut bool,
    floppy_b_present: &mut bool,
    menu: &mut AppMenu,
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
        );
    }
}

fn load_and_insert_disk(
    slot: DriveNumber,
    path: &str,
    computer: &mut Computer<GuiKeyboard, PixelsVideoController>,
    floppy_a_present: &mut bool,
    floppy_b_present: &mut bool,
    menu: &mut AppMenu,
) {
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
        }
        Err(e) => {
            log::error!("Failed to insert disk: {}", e);
        }
    }
}

fn eject_disk(
    slot: DriveNumber,
    computer: &mut Computer<GuiKeyboard, PixelsVideoController>,
    floppy_a_present: &mut bool,
    floppy_b_present: &mut bool,
    menu: &mut AppMenu,
) {
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
        }
        Ok(None) => {
            log::warn!("No disk in floppy {} to eject", slot);
        }
        Err(e) => {
            log::error!("Failed to eject disk: {}", e);
        }
    }
}
