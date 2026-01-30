mod font;
mod gui_keyboard;
mod gui_video;

use anyhow::{Context, Result};
use clap::Parser;
use emu86_core::utils::parse_hex_or_dec;
use emu86_core::{
    BackedDisk, Bios, Computer, DiskController, DriveNumber, FileDiskBackend, PartitionedDisk,
    parse_mbr,
};
use gui_keyboard::GuiKeyboard;
use gui_video::{PixelsVideoController, SCREEN_HEIGHT, SCREEN_WIDTH};
use pixels::{Pixels, SurfaceTexture};
use std::fs::File;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

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

fn main() -> Result<()> {
    let log_file = File::create("/tmp/emu86.log").context("Failed to create log file")?;
    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    let cli = Cli::parse();

    // Create event loop
    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(cli)?;
    event_loop.run_app(&mut app)?;

    Ok(())
}

struct App {
    cli: Cli,
    state: Option<AppState>,
}

struct AppState {
    window: &'static Window,
    pixels: Pixels<'static>,
    computer: Computer<GuiKeyboard, PixelsVideoController>,
}

impl App {
    fn new(cli: Cli) -> Result<Self> {
        Ok(Self { cli, state: None })
    }

    fn create_computer(&self) -> Result<Computer<GuiKeyboard, PixelsVideoController>> {
        // Create BIOS with no drives attached
        let keyboard = GuiKeyboard::new();
        let mut bios: Bios<GuiKeyboard> = Bios::new(keyboard);

        // Load floppy A:
        if let Some(path) = &self.cli.floppy_a {
            let backend = FileDiskBackend::open(path, false)?;
            let disk = BackedDisk::new(backend)
                .with_context(|| format!("Failed to create disk from: {}", path))?;
            bios.insert_floppy(DriveNumber::floppy_a(), Box::new(disk))
                .map_err(|e| anyhow::anyhow!("Failed to insert floppy A:: {}", e))?;
            log::info!("Opened floppy A: from {}", path);
        }

        // Load floppy B:
        if let Some(path) = &self.cli.floppy_b {
            let backend = FileDiskBackend::open(path, false)?;
            let disk = BackedDisk::new(backend)
                .with_context(|| format!("Failed to create disk from: {}", path))?;
            bios.insert_floppy(DriveNumber::floppy_b(), Box::new(disk))
                .map_err(|e| anyhow::anyhow!("Failed to insert floppy B:: {}", e))?;
            log::info!("Opened floppy B: from {}", path);
        }

        // Load hard drives (C:, D:, etc.)
        for path in self.cli.hard_disks.iter() {
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
                bios.add_hard_drive_with_partition(Box::new(partitioned), Box::new(raw_disk))
            } else {
                log::info!("No MBR detected on {}, using raw disk", path);
                bios.add_hard_drive(Box::new(disk))
            };

            log::info!(
                "Opened hard drive {}: ({}) from {}",
                drive_num.to_letter(),
                drive_num,
                path
            );
        }

        // If no drives specified and booting, error out
        if self.cli.floppy_a.is_none()
            && self.cli.floppy_b.is_none()
            && self.cli.hard_disks.is_empty()
            && self.cli.boot
        {
            return Err(anyhow::anyhow!(
                "No disk images specified. Use --floppy-a, --floppy-b, or --hdd to specify disk images."
            ));
        }

        let video = PixelsVideoController::new();
        let mut computer = Computer::new(bios, video);

        if self.cli.boot {
            // Boot from disk
            let boot_drive = parse_hex_or_dec(&self.cli.boot_drive)?;
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
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        // Create window
        let window_attrs = Window::default_attributes()
            .with_title("emu86 - 8086 Emulator")
            .with_inner_size(LogicalSize::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32))
            .with_resizable(true);

        let window = event_loop
            .create_window(window_attrs)
            .expect("Failed to create window");

        // Create pixels surface
        // Leak the window to get a 'static reference for SurfaceTexture
        // This is safe because the window will live for the entire duration of the program
        let window_ref: &'static Window = Box::leak(Box::new(window));
        let window_size = window_ref.inner_size();
        let surface_texture =
            SurfaceTexture::new(window_size.width, window_size.height, window_ref);
        let pixels = Pixels::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface_texture)
            .expect("Failed to create Pixels");

        // Initialize computer
        let computer = match self.create_computer() {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to initialize computer: {}", e);
                event_loop.exit();
                return;
            }
        };

        self.state = Some(AppState {
            window: window_ref,
            pixels,
            computer,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                log::info!("Window close requested");
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Err(e) = state.pixels.resize_surface(new_size.width, new_size.height) {
                    log::error!("Failed to resize surface: {}", e);
                    event_loop.exit();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                state.computer.bios_mut().keyboard.process_event(&event);
            }
            WindowEvent::RedrawRequested => {
                // Execute a batch of instructions
                const BATCH_SIZE: u32 = 1000;
                for _ in 0..BATCH_SIZE {
                    if state.computer.is_halted() {
                        log::info!("Computer halted");
                        event_loop.exit();
                        break;
                    }
                    state.computer.step();
                }

                // Update video from emulator state
                state.computer.update_video();

                // Render to pixels buffer
                state
                    .computer
                    .video_controller_mut()
                    .render(&mut state.pixels);

                // Present to window
                if let Err(e) = state.pixels.render() {
                    log::error!("Failed to render: {}", e);
                    event_loop.exit();
                }

                // Request next frame
                state.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request redraw to keep the emulation running continuously
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}
