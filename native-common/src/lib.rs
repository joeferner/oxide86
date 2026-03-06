pub mod cli;
pub mod clock;
pub mod disk;
pub mod logging;
pub mod rodio_pc_speaker;

use std::sync::{Arc, RwLock};

use anyhow::{Context, Result, anyhow};
use oxide86_core::{
    computer::{Computer, ComputerConfig},
    cpu::CpuType,
    devices::{
        pc_speaker::{NullPcSpeaker, PcSpeaker},
        serial_mouse::SerialMouse,
        uart::ComPortDevice,
    },
    disk::{BackedDisk, Disk, DriveNumber},
    parse_hex_or_dec,
    video::{VideoBuffer, VideoCardType},
};

use crate::{
    cli::CommonCli, clock::NativeClock, disk::FileDiskBackend, rodio_pc_speaker::RodioPcSpeaker,
};
use rodio::{DeviceSinkBuilder, MixerDeviceSink};

pub fn create_computer(
    cli: &CommonCli,
    video_buffer: Arc<RwLock<VideoBuffer>>,
    native_mouse: Arc<RwLock<SerialMouse>>,
) -> Result<(Computer, Option<MixerDeviceSink>)> {
    let cpu_type = if let Some(cpu_type) = CpuType::parse(&cli.cpu_type) {
        cpu_type
    } else {
        return Err(anyhow!("Could not parse CPU type: {}", cli.cpu_type));
    };

    let hard_disks = load_hard_disks(&cli.hard_disks);
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

    let mut computer = Computer::new(ComputerConfig {
        cpu_type,
        clock_speed: (cli.speed * 1_000_000.0) as u32,
        memory_size: parse_memory(&cli.memory)?,
        clock: Box::new(NativeClock::new()),
        hard_disks,
        video_card_type,
        video_buffer,
        pc_speaker,
    });
    if cli.exec_log {
        computer.set_exec_logging_enabled(true);
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

    computer.set_com_port_device(1, create_com_device(&cli.com1_device, &native_mouse)?);
    computer.set_com_port_device(2, create_com_device(&cli.com2_device, &native_mouse)?);
    computer.set_com_port_device(3, create_com_device(&cli.com3_device, &native_mouse)?);
    computer.set_com_port_device(4, create_com_device(&cli.com4_device, &native_mouse)?);

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

    Ok((computer, sink))
}

fn create_com_device(
    device_name: &Option<String>,
    native_mouse: &Arc<RwLock<SerialMouse>>,
) -> Result<Option<Arc<RwLock<dyn ComPortDevice>>>> {
    if let Some(device_name) = device_name {
        let device_name = device_name.trim().to_lowercase();
        if device_name == "none" || device_name == "null" {
            Ok(None)
        } else if device_name == "mouse" {
            Ok(Some(native_mouse.clone()))
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

fn load_hard_disks(hard_disks: &[String]) -> Vec<Box<dyn Disk>> {
    hard_disks
        .iter()
        .enumerate()
        .filter_map(|(i, path)| {
            let backend = FileDiskBackend::open(path, false)
                .map_err(|e| log::error!("Failed to open hard drive {}: {}", path, e))
                .ok()?;
            let disk = BackedDisk::new(backend)
                .map_err(|e| log::error!("Failed to create disk from {}: {}", path, e))
                .ok()?;
            let letter = DriveNumber::from_hard_drive_index(i).to_letter();
            log::info!("Opened hard drive {}: from {}", letter, path);
            Some(Box::new(disk) as Box<dyn Disk>)
        })
        .collect()
}
