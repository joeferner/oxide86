use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use oxide86_core::{
    Devices,
    computer::Computer,
    cpu::{Cpu, CpuType},
    disk::{BackedDisk, DiskController, DriveNumber},
    io_bus::IoBus,
    memory::Memory,
    memory_bus::MemoryBus,
    parse_hex_or_dec,
    video::{VideoBuffer, VideoCard},
};

use crate::{cli::CommonCli, disk::FileDiskBackend};

pub mod cli;
pub mod disk;
pub mod logging;

pub fn create_computer(cli: &CommonCli, buffer: Arc<VideoBuffer>) -> Result<Computer> {
    let cpu_type = if let Some(cpu_type) = CpuType::parse(&cli.cpu_type) {
        cpu_type
    } else {
        return Err(anyhow!("Could not parse CPU type: {}", cli.cpu_type));
    };

    let cpu = {
        let mut cpu = Cpu::new(cpu_type);
        if cli.exec_log {
            cpu.exec_logging_enabled = true;
        }
        cpu
    };

    let memory = Memory::new(2048 * 1024); // TODO fill from cli args

    let devices = {
        let mut devices = Devices::new();
        devices.push(VideoCard::new(buffer));

        // Load floppy A:
        if let Some(path) = &cli.floppy_a {
            let backend = FileDiskBackend::open(path, false)?;
            let disk = BackedDisk::new(backend)
                .with_context(|| format!("Failed to create disk from: {}", path))?;
            let controller = DiskController::new(DriveNumber::floppy_a(), Box::new(disk));
            devices.push(controller);
            log::info!("Opened floppy A: from {}", path);
        }

        devices
    };

    let memory_bus = MemoryBus::new(memory, devices.clone());
    let io_bus = IoBus::new(devices);
    let mut computer = Computer::new(cpu, memory_bus, io_bus);

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
        if cli.floppy_a.is_none()
        /* TODO && cli.floppy_b.is_none() && cli.hard_disks.is_empty() && cli.boot */
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
