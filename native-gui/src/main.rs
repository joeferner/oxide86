mod font;
mod gui_keyboard;
mod gui_video;
mod menu;

use anyhow::{Context, Result};
use clap::Parser;
use emu86_core::utils::parse_hex_or_dec;
use emu86_core::{
    BackedDisk, Bios, Computer, DiskController, DriveNumber, FileDiskBackend, PartitionedDisk,
    parse_mbr,
};
use gui_keyboard::GuiKeyboard;
use gui_video::{PixelsVideoController, SCREEN_HEIGHT, SCREEN_WIDTH};
use menu::{AppEvent, AppMenu};
use pixels::{Pixels, SurfaceTexture};
use std::fs::File;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
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

    // Create event loop with custom event type
    let event_loop = EventLoop::<AppEvent>::with_user_event()
        .build()
        .context("Failed to create event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);

    // Get event loop proxy
    let event_proxy = event_loop.create_proxy();

    // Set up muda event handler (must be before event loop starts)
    muda::MenuEvent::set_event_handler(Some({
        let proxy = event_proxy.clone();
        move |event: muda::MenuEvent| {
            let _ = proxy.send_event(AppEvent::MenuEvent(event));
        }
    }));

    let mut app = App::new(cli, event_proxy)?;
    event_loop.run_app(&mut app)?;

    Ok(())
}

struct App {
    cli: Cli,
    state: Option<AppState>,
    menu: Option<AppMenu>,
    #[allow(dead_code)]
    event_proxy: EventLoopProxy<AppEvent>,
    floppy_a_present: bool,
    floppy_b_present: bool,
}

struct AppState {
    window: &'static Window,
    pixels: Pixels<'static>,
    computer: Computer<GuiKeyboard, PixelsVideoController>,
}

impl App {
    fn new(cli: Cli, event_proxy: EventLoopProxy<AppEvent>) -> Result<Self> {
        Ok(Self {
            cli,
            state: None,
            menu: None,
            event_proxy,
            floppy_a_present: false,
            floppy_b_present: false,
        })
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

    fn handle_menu_event(&mut self, menu_event: muda::MenuEvent) {
        let Some(menu) = &self.menu else {
            return;
        };

        let Some(action) = menu.get_menu_action(&menu_event) else {
            log::warn!("Unknown menu event: {:?}", menu_event.id());
            return;
        };

        if action.is_insert() {
            self.show_insert_dialog(action.drive_number());
        } else {
            self.eject_disk(action.drive_number());
        }
    }

    fn show_insert_dialog(&mut self, slot: DriveNumber) {
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
            self.load_and_insert_disk(slot, &path);
        }
    }

    fn load_and_insert_disk(&mut self, slot: DriveNumber, path: &str) {
        let result = (|| -> Result<()> {
            let backend = FileDiskBackend::open(path, false)?;
            let disk = BackedDisk::new(backend)
                .with_context(|| format!("Invalid disk image: {}", path))?;

            let state = self.state.as_mut().unwrap();
            state
                .computer
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
                    self.floppy_a_present = true;
                } else {
                    self.floppy_b_present = true;
                }
                // Update menu
                if let Some(menu) = &self.menu {
                    menu.update_menu_states(self.floppy_a_present, self.floppy_b_present);
                }
            }
            Err(e) => {
                log::error!("Failed to insert disk: {}", e);
            }
        }
    }

    fn eject_disk(&mut self, slot: DriveNumber) {
        let state = self.state.as_mut().unwrap();
        match state.computer.bios_mut().eject_floppy(slot) {
            Ok(Some(_disk)) => {
                log::info!("Ejected floppy {}", slot);
                // Update state
                if slot == DriveNumber::floppy_a() {
                    self.floppy_a_present = false;
                } else {
                    self.floppy_b_present = false;
                }
                // Update menu
                if let Some(menu) = &self.menu {
                    menu.update_menu_states(self.floppy_a_present, self.floppy_b_present);
                }
            }
            Ok(None) => {
                log::warn!("No disk in floppy {} to eject", slot);
            }
            Err(e) => {
                log::error!("Failed to eject disk: {}", e);
            }
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
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

        // Create menu
        let menu = match menu::create_menu() {
            Ok(m) => m,
            Err(e) => {
                log::error!("Failed to create menu: {}", e);
                event_loop.exit();
                return;
            }
        };

        // Initialize menu for window (platform-specific)
        #[cfg(target_os = "windows")]
        {
            use winit::raw_window_handle::HasWindowHandle;
            match window_ref.window_handle() {
                Ok(handle) => {
                    if let raw_window_handle::RawWindowHandle::Win32(h) = handle.as_raw() {
                        if let Err(e) = menu.menu.init_for_hwnd(h.hwnd.get() as isize) {
                            log::error!("Failed to init menu for Windows: {}", e);
                            event_loop.exit();
                            return;
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to get window handle: {}", e);
                    event_loop.exit();
                    return;
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Linux menu initialization may require GTK window handle
            log::info!("Linux menu initialization - using default setup");
        }

        #[cfg(target_os = "macos")]
        {
            if let Err(e) = menu.menu.init_for_nsapp() {
                log::error!("Failed to init menu for macOS: {}", e);
                event_loop.exit();
                return;
            }
        }

        // Check initial disk presence from CLI args
        let floppy_a_present = self.cli.floppy_a.is_some();
        let floppy_b_present = self.cli.floppy_b.is_some();

        // Update menu states
        menu.update_menu_states(floppy_a_present, floppy_b_present);

        // Store menu and disk presence state
        self.menu = Some(menu);
        self.floppy_a_present = floppy_a_present;
        self.floppy_b_present = floppy_b_present;

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
            WindowEvent::ModifiersChanged(modifiers) => {
                let mod_state = modifiers.state();
                state
                    .computer
                    .bios_mut()
                    .keyboard
                    .update_modifiers(mod_state);

                // Update BDA keyboard flags so INT 16h AH=02h works correctly
                state.computer.update_keyboard_flags(
                    mod_state.shift_key(),
                    mod_state.control_key(),
                    mod_state.alt_key(),
                );
            }
            WindowEvent::RedrawRequested => {
                const BATCH_SIZE: u32 = 10000;

                for _ in 0..BATCH_SIZE {
                    if state.computer.is_halted() {
                        log::info!("Computer halted");
                        event_loop.exit();
                        break;
                    }
                    state.computer.step();
                }

                // Update video from emulator state (only updates if video is dirty)
                state.computer.update_video();

                // Only render if there are actual updates to display
                if state.computer.video_controller_mut().has_pending_updates() {
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
                }

                // Request next frame to keep emulation running
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

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::MenuEvent(menu_event) => {
                self.handle_menu_event(menu_event);
            }
            AppEvent::DiskInserted { slot, result } => match result {
                Ok(()) => {
                    log::info!("Disk inserted successfully into {}", slot);
                }
                Err(e) => {
                    log::error!("Failed to insert disk into {}: {}", slot, e);
                }
            },
        }
    }
}
