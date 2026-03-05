use std::sync::{Arc, RwLock};

use anyhow::{Context, Result, anyhow};
use oxide86_core::{
    computer::{Computer, ComputerConfig},
    cpu::CpuType,
    disk::{BackedDisk, Disk, DriveNumber},
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

    let hard_disks = load_hard_disks(&cli.hard_disks);

    let mut computer = Computer::new(ComputerConfig {
        cpu_type,
        clock_speed: (cli.speed * 1_000_000.0) as u32,
        memory_size: parse_memory(&cli.memory)?,
        clock: Box::new(NativeClock::new()),
        hard_disks,
    });
    if cli.exec_log {
        computer.set_exec_logging_enabled(true);
    }

    computer.add_device(VideoCard::new(buffer));

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

    Ok(computer)
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
fn parse_disk_spec(spec: &str) -> (&str, bool) {
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
