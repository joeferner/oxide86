use std::sync::{Arc, RwLock};

use anyhow::{Context, Result, anyhow};
use oxide86_core::{
    computer::{Computer, ComputerConfig},
    cpu::CpuType,
    disk::{BackedDisk, DriveNumber},
    parse_hex_or_dec,
    video::{VideoBuffer, VideoCard},
};

use crate::{cli::CommonCli, clock::NativeClock, disk::FileDiskBackend};

pub mod cli;
pub mod clock;
pub mod disk;
pub mod logging;

pub fn create_computer(cli: &CommonCli, buffer: Arc<RwLock<VideoBuffer>>) -> Result<Computer> {
    let cpu_type = if let Some(cpu_type) = CpuType::parse(&cli.cpu_type) {
        cpu_type
    } else {
        return Err(anyhow!("Could not parse CPU type: {}", cli.cpu_type));
    };

    let mut computer = Computer::new(ComputerConfig {
        cpu_type,
        clock_speed: (cli.speed * 1_000_000.0) as u32,
        memory_size: 2048 * 1024, // TODO fill from cli args
        clock: Box::new(NativeClock::new()),
    });
    if cli.exec_log {
        computer.set_exec_logging_enabled(true);
    }

    computer.add_device(VideoCard::new(buffer));

    // Load floppy A:
    if let Some(path) = &cli.floppy_a {
        let backend = FileDiskBackend::open(path, false)?;
        let disk = BackedDisk::new(backend)
            .with_context(|| format!("Failed to create disk from: {}", path))?;
        computer.set_floppy_disk(DriveNumber::floppy_a(), Box::new(disk));
        log::info!("Opened floppy A: from {}", path);
    }

    // Load floppy B:
    if let Some(path) = &cli.floppy_b {
        let backend = FileDiskBackend::open(path, false)?;
        let disk = BackedDisk::new(backend)
            .with_context(|| format!("Failed to create disk from: {}", path))?;
        computer.set_floppy_disk(DriveNumber::floppy_b(), Box::new(disk));
        log::info!("Opened floppy B: from {}", path);
    }

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
        if cli.floppy_a.is_none() && cli.floppy_b.is_none()
        /* TODO  && cli.hard_disks.is_empty() && cli.boot */
        {
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

    Ok(computer)
}
