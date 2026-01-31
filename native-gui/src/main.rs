mod font;
mod gui_keyboard;
mod gui_video;
mod menu;

use anyhow::{Context, Result};
use clap::Parser;
use emu86_core::utils::parse_hex_or_dec;
use emu86_core::{
    BackedDisk, Computer, DiskController, DriveNumber, FileDiskBackend, NullMouse, PartitionedDisk,
    parse_mbr,
};
use gui_keyboard::GuiKeyboard;
use gui_video::{PixelsVideoController, SCREEN_HEIGHT, SCREEN_WIDTH};
use menu::AppMenu;
use pixels::{Pixels, SurfaceTexture, wgpu};
use std::fs::File;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

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
}

fn main() {
    let log_file = File::create("emu86.log").expect("Failed to create log file");
    env_logger::Builder::from_default_env()
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

    // Add extra height for the menu bar
    // Since the scaling renderer centers content with letterboxing,
    // we need 2x the menu height as extra space (menu height top + bottom letterbox)
    const MENU_HEIGHT: u32 = 30;

    let window = WindowBuilder::new()
        .with_title("emu86 - 8086 Emulator")
        .with_inner_size(LogicalSize::new(
            SCREEN_WIDTH as u32,
            SCREEN_HEIGHT as u32 + (MENU_HEIGHT * 2),
        ))
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

    // Initialize computer
    let mut computer = create_computer(&cli)?;

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

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            if let Event::WindowEvent { event, .. } = event {
                // Let egui handle the event first
                let response = egui_state.on_window_event(window, &event);
                if response.consumed {
                    return;
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
                    }
                    WindowEvent::KeyboardInput { event: input, .. } => {
                        computer.bios_mut().keyboard.process_event(&input);
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

                        // Render egui (menu bar and central panel for emulator)
                        let raw_input = egui_state.take_egui_input(window);
                        let full_output = egui_ctx.run(raw_input, |ctx| {
                            let action = menu.render(ctx);

                            // Add a central panel to reserve space for the emulator screen
                            // This ensures the menu bar doesn't overlap the emulator display
                            egui::CentralPanel::default()
                                .frame(egui::Frame::none())
                                .show(ctx, |ui| {
                                    // Reserve exact space for the emulator
                                    ui.allocate_space(egui::vec2(
                                        SCREEN_WIDTH as f32,
                                        SCREEN_HEIGHT as f32,
                                    ));
                                });

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
                            // The scaling renderer will center the 640x400 content in the 640x460 window,
                            // creating 30px letterbox at top and bottom, which perfectly aligns with the menu
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

fn create_computer(cli: &Cli) -> Result<Computer<GuiKeyboard, PixelsVideoController>> {
    // Create computer with keyboard and mouse
    let keyboard = GuiKeyboard::new();
    let mouse = Box::new(NullMouse::new());
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
