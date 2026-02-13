use anyhow::{Context, Result};
use emu86_core::utils::parse_hex_or_dec;
use emu86_core::{
    BackedDisk, Computer, DiskController, DriveNumber, FileDiskBackend, MouseInput, NullSpeaker,
    PartitionedDisk, RodioSpeaker, SerialMouse, SpeakerOutput, VideoController, parse_mbr,
};

use crate::CommonCli;

/// Create a speaker with Rodio, falling back to NullSpeaker if unavailable.
pub fn create_speaker() -> Box<dyn SpeakerOutput> {
    match RodioSpeaker::new() {
        Ok(rodio_speaker) => {
            log::info!("PC speaker enabled (Rodio)");
            Box::new(rodio_speaker)
        }
        Err(e) => {
            log::warn!("PC speaker unavailable: {}", e);
            log::info!("Using NullSpeaker (no audio)");
            Box::new(NullSpeaker)
        }
    }
}

/// Load floppy and hard disk images into the computer.
pub fn load_disks<V: VideoController>(
    computer: &mut Computer<V>,
    floppy_a: &Option<String>,
    floppy_b: &Option<String>,
    hard_disks: &[String],
) -> Result<()> {
    // Load floppy A:
    if let Some(path) = floppy_a {
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
    if let Some(path) = floppy_b {
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
    for path in hard_disks.iter() {
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

    // Update BDA hard drive count after adding all drives
    if !hard_disks.is_empty() {
        computer.update_bda_hard_drive_count();
    }

    Ok(())
}

/// Load a program or boot from disk based on CLI arguments.
pub fn load_program_or_boot<V: VideoController>(
    computer: &mut Computer<V>,
    cli: &CommonCli,
) -> Result<()> {
    // Validate that drives are specified if booting
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
    } else if let Some(program_path) = &cli.program {
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
    }

    Ok(())
}

/// Attach a serial device (e.g., mouse) to a COM port.
pub fn attach_serial_device<V: VideoController>(
    computer: &mut Computer<V>,
    port: u8,
    device_name: &str,
    mouse: Box<dyn MouseInput>,
) {
    match device_name {
        "mouse" => {
            let serial_mouse = Box::new(SerialMouse::new(mouse));
            match port {
                1 => {
                    computer.set_com1_device(serial_mouse);
                    log::info!("Serial mouse attached to COM1");
                }
                2 => {
                    computer.set_com2_device(serial_mouse);
                    log::info!("Serial mouse attached to COM2");
                }
                _ => {
                    eprintln!("Warning: Invalid COM port {}", port);
                }
            }
        }
        _ => {
            eprintln!("Warning: Unknown device '{}' for COM{}", device_name, port);
        }
    }
}

/// Apply execution and interrupt logging flags.
pub fn apply_logging_flags<V: VideoController>(
    computer: &mut Computer<V>,
    exec_log: bool,
    int_log: bool,
) {
    if exec_log {
        computer.set_exec_logging(true);
        log::info!("Execution logging enabled");
    }
    if int_log {
        computer.set_log_interrupts(true);
        log::info!("Interrupt logging enabled");
    }
}
