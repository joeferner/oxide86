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




struct ComputerSetup {
    computer: Computer<PixelsVideoController>,
    gilrs_joystick: Option<GilrsJoystick>,
    mounted_drives: Vec<DriveNumber>,
    audio_output: Option<AudioOutput>,
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
