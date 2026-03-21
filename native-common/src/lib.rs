pub mod cli;
pub mod clock;
pub mod disk;
pub mod gilrs_joystick;
pub mod logging;
pub mod rodio_pc_speaker;
pub mod rodio_sound_card;
pub mod throttle;

use std::sync::{Arc, RwLock};

use anyhow::{Context, Result, anyhow};
use oxide86_core::debugger::DebugShared;
use oxide86_core::{
    computer::{Computer, ComputerConfig},
    cpu::CpuType,
    devices::{
        SoundCardType,
        adlib::Adlib,
        pc_speaker::{NullPcSpeaker, PcSpeaker},
        serial_mouse::SerialMouse,
        uart::ComPortDevice,
    },
    disk::{BackedDisk, Disk, DriveNumber},
    parse_hex_or_dec,
    video::{VideoBuffer, VideoCardType},
};

use crate::{
    cli::CommonCli, clock::NativeClock, disk::FileDiskBackend, gilrs_joystick::GilrsJoystick,
    rodio_pc_speaker::RodioPcSpeaker, rodio_sound_card::RodioSoundCard,
};
use rodio::{DeviceSinkBuilder, MixerDeviceSink};

pub fn has_com_mouse(cli: &CommonCli) -> Result<bool> {
    let com_has_mouse = [
        &cli.com1_device,
        &cli.com2_device,
        &cli.com3_device,
        &cli.com4_device,
    ]
    .iter()
    .any(|d| d.as_deref() == Some("mouse"));
    if cli.ps2_mouse && com_has_mouse {
        Err(anyhow!(
            "cannot use --ps2-mouse together with a serial mouse on a COM port"
        ))
    } else {
        Ok(com_has_mouse)
    }
}

pub fn create_computer(
    cli: &CommonCli,
    video_buffer: Arc<RwLock<VideoBuffer>>,
    serial_mouse: Option<Arc<RwLock<SerialMouse>>>,
) -> Result<(Computer, Option<MixerDeviceSink>, Option<GilrsJoystick>)> {
    let cpu_type = if let Some(cpu_type) = CpuType::parse(&cli.cpu_type) {
        cpu_type
    } else {
        return Err(anyhow!("Could not parse CPU type: {}", cli.cpu_type));
    };

    let hard_disks = load_hard_disks(&cli.hard_disks)?;
    let video_card_type = if let Some(video_card_type) = VideoCardType::parse(&cli.video_card) {
        video_card_type
    } else {
        return Err(anyhow!(
            "Could not parse video card type: {}",
            cli.video_card
        ));
    };

    let (sink, pc_speaker) = match DeviceSinkBuilder::open_default_sink() {
        Ok(sink) => {
            let pc_speaker = create_pc_speaker(&sink, !cli.disable_pc_speaker);
            (Some(sink), pc_speaker as Box<dyn PcSpeaker>)
        }
        Err(e) => {
            log::warn!("Audio device unavailable: {}", e);
            (None, Box::new(NullPcSpeaker::new()) as Box<dyn PcSpeaker>)
        }
    };

    let cpu_freq = (cli.speed * 1_000_000.0) as u64;

    let mut computer = Computer::new(ComputerConfig {
        cpu_type,
        clock_speed: cpu_freq as u32,
        memory_size: parse_memory(&cli.memory)?,
        clock: Box::new(NativeClock::new()),
        hard_disks,
        video_card_type,
        video_buffer,
        pc_speaker,
        math_coprocessor: !cli.no_fpu,
    });

    if let Some(sound_card_type) = SoundCardType::parse(&cli.sound_card) {
        match sound_card_type {
            SoundCardType::None => {}
            SoundCardType::AdLib => {
                let adlib = Adlib::new(cpu_freq);
                if let Some(sink) = &sink {
                    sink.mixer().add(RodioSoundCard::new(adlib.consumer()));
                }
                computer.add_sound_card(adlib);
            }
        }
    } else {
        return Err(anyhow!(
            "Could not parse sound card type: {}",
            cli.sound_card
        ));
    }

    if let Some(port) = cli.debug_mcp_port {
        let debug = Arc::new(DebugShared::new());
        if cli.debug_mcp_pause_on_start {
            debug.pause_requested.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        computer.set_debug(Arc::clone(&debug));
        oxide86_debugger::server::start_mcp_server(port, debug);
        log::info!("MCP debug server started on port {port}");
    }

    if cli.exec_log {
        computer.set_exec_logging_enabled(true);
    }

    if !cli.watch.is_empty() {
        let mut addrs = Vec::with_capacity(cli.watch.len());
        for s in &cli.watch {
            let trimmed = s.trim().trim_start_matches("0x").trim_start_matches("0X");
            let addr = usize::from_str_radix(trimmed, 16)
                .map_err(|_| anyhow!("Invalid watch address: {s}"))?;
            log::info!("Watching writes to physical address 0x{addr:05X}");
            addrs.push(addr);
        }
        computer.set_watch_addresses(addrs);
    }

    // Load floppy A:
    if let Some(spec) = &cli.floppy_a {
        let (path, read_only) = parse_disk_spec(spec);
        let backend = FileDiskBackend::open(path, read_only)?;
        let disk = BackedDisk::new(backend)
            .with_context(|| format!("Failed to create disk from: {}", path))?;
        computer.set_floppy_disk(DriveNumber::floppy_a(), Some(Box::new(disk)));
        log::info!("Opened floppy A: from {} (read_only={})", path, read_only);
    }

    // Load floppy B:
    if let Some(spec) = &cli.floppy_b {
        let (path, read_only) = parse_disk_spec(spec);
        let backend = FileDiskBackend::open(path, read_only)?;
        let disk = BackedDisk::new(backend)
            .with_context(|| format!("Failed to create disk from: {}", path))?;
        computer.set_floppy_disk(DriveNumber::floppy_b(), Some(Box::new(disk)));
        log::info!("Opened floppy B: from {} (read_only={})", path, read_only);
    }

    computer.set_com_port_device(1, create_com_device(&cli.com1_device, &serial_mouse)?);
    computer.set_com_port_device(2, create_com_device(&cli.com2_device, &serial_mouse)?);
    computer.set_com_port_device(3, create_com_device(&cli.com3_device, &serial_mouse)?);
    computer.set_com_port_device(4, create_com_device(&cli.com4_device, &serial_mouse)?);

    if let Some(program_path) = &cli.program {
        // Load program from file
        let program_data = std::fs::read(program_path)
            .with_context(|| format!("Failed to read program file: {}", program_path))?;

        let segment = parse_hex_or_dec(&cli.segment)?;
        let offset = parse_hex_or_dec(&cli.offset)?;

        computer
            .load_program(&program_data, segment, offset)
            .context("Failed to load program")?;

        log::info!(
            "Loaded {} bytes at {:04X}:{:04X}",
            program_data.len(),
            segment,
            offset
        );
    } else {
        // Validate that drives are specified if booting
        if cli.floppy_a.is_none() && cli.floppy_b.is_none() && cli.hard_disks.is_empty() {
            return Err(anyhow::anyhow!(
                "No disk images specified. Use --floppy-a, --floppy-b, or --hdd to specify disk images."
            ));
        }

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
    }

    let gilrs_joystick = if cli.joystick {
        GilrsJoystick::new()
    } else {
        None
    };

    Ok((computer, sink, gilrs_joystick))
}

fn create_com_device(
    device_name: &Option<String>,
    serial_mouse: &Option<Arc<RwLock<SerialMouse>>>,
) -> Result<Option<Arc<RwLock<dyn ComPortDevice>>>> {
    if let Some(device_name) = device_name {
        let device_name = device_name.trim().to_lowercase();
        if device_name == "none" || device_name == "null" {
            Ok(None)
        } else if device_name == "mouse" {
            if let Some(native_mouse) = serial_mouse {
                Ok(Some(native_mouse.clone()))
            } else {
                Err(anyhow!("Serial mouse not initialized"))
            }
        } else {
            Err(anyhow!("Invalid COM device name: {device_name}"))
        }
    } else {
        Ok(None)
    }
}

fn create_pc_speaker(sink: &MixerDeviceSink, enabled: bool) -> Box<dyn PcSpeaker> {
    if enabled {
        Box::new(RodioPcSpeaker::new(sink))
    } else {
        Box::new(NullPcSpeaker::new())
    }
}

fn parse_memory(memory: &str) -> Result<usize> {
    let mem = memory.trim().to_lowercase();
    if mem.ends_with("mb") || mem.ends_with("m") {
        let mem = mem.trim_end_matches("mb").trim_end_matches("m");
        let mb = mem
            .parse::<usize>()
            .map_err(|_| anyhow!("Could not parse memory: {memory}"))?;
        Ok(mb * 1024 * 1024)
    } else {
        let mem = mem.trim_end_matches("kb").trim_end_matches("k");
        let mb = mem
            .parse::<usize>()
            .map_err(|_| anyhow!("Could not parse memory: {memory}"))?;
        Ok(mb * 1024)
    }
}

/// Parse a disk spec like "path/to/disk.img" or "path/to/disk.img:r" (read-only).
pub fn parse_disk_spec(spec: &str) -> (&str, bool) {
    if let Some(path) = spec.strip_suffix(":r") {
        (path, true)
    } else {
        (spec, false)
    }
}

fn load_hard_disks(hard_disks: &[String]) -> Result<Vec<Box<dyn Disk>>> {
    hard_disks
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let backend = FileDiskBackend::open(path, false)
                .with_context(|| format!("Failed to open hard drive: {}", path))?;
            let disk = BackedDisk::new(backend)
                .with_context(|| format!("Failed to create disk from: {}", path))?;
            let letter = DriveNumber::from_hard_drive_index(i).to_letter();
            log::info!("Opened hard drive {}: from {}", letter, path);
            Ok(Box::new(disk) as Box<dyn Disk>)
        })
        .collect()
}
