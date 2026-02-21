use anyhow::{Context, Result};
use emu86_core::audio::adlib::{ADLIB_SAMPLE_RATE, Adlib};
use emu86_core::utils::parse_hex_or_dec;
use emu86_core::{
    BackedDisk, CdRomImage, Computer, DiskController, DriveNumber, MouseInput, NullSpeaker,
    PartitionedDisk, SerialLogger, SerialMouse, SoundCard, SpeakerOutput, VideoController,
    parse_mbr,
};
use rodio::OutputStreamBuilder;
use rodio::stream::OutputStream;

use crate::{CommonCli, FileDiskBackend, HostDirectoryDisk, RodioPcm, RodioSpeaker};
use std::path::PathBuf;

/// Keeps a single Rodio output stream and all audio sinks alive.
///
/// Must be held for the duration of emulation; dropping it stops all audio.
pub struct AudioOutput {
    _stream: OutputStream,
    _pcm_sink: Option<RodioPcm>,
}

/// Result of [`create_audio`]: speaker, optional sound card, and the stream/sink bundle.
pub type AudioSetup = (
    Box<dyn SpeakerOutput>,
    Option<Box<dyn SoundCard>>,
    Option<AudioOutput>,
);

/// Create all audio outputs (PC speaker + optional sound card) from a single stream.
///
/// Returns `(speaker, sound_card, audio_output)`. The caller must:
/// - pass `speaker` to `Computer::new()`
/// - call `computer.set_sound_card(card)` if `sound_card` is `Some`
/// - keep `audio_output` alive for the duration of emulation
pub fn create_audio(speaker_enabled: bool, sound_card: &str, cpu_freq: u64) -> AudioSetup {
    let is_adlib = matches!(sound_card.to_lowercase().trim(), "adlib" | "adl");

    if !speaker_enabled {
        log::info!("PC speaker disabled (--disable-pc-speaker)");
    }
    if !is_adlib {
        log::info!("AdLib disabled (--sound-card={})", sound_card);
    }

    if !speaker_enabled && !is_adlib {
        return (Box::new(NullSpeaker), None, None);
    }

    let stream = match OutputStreamBuilder::open_default_stream() {
        Ok(s) => s,
        Err(e) => {
            log::warn!("Audio device unavailable: {}", e);
            return (Box::new(NullSpeaker), None, None);
        }
    };

    let speaker: Box<dyn SpeakerOutput> = if speaker_enabled {
        log::info!("PC speaker enabled (Rodio)");
        Box::new(RodioSpeaker::new(&stream))
    } else {
        Box::new(NullSpeaker)
    };

    let (sound_card, pcm_sink) = if is_adlib {
        let adlib = Adlib::new(cpu_freq);
        let consumer = adlib.consumer();
        let sink = RodioPcm::new(consumer, &stream);
        log::info!("AdLib (OPL2) enabled (Rodio, {} Hz)", ADLIB_SAMPLE_RATE);
        (Some(Box::new(adlib) as Box<dyn SoundCard>), Some(sink))
    } else {
        (None, None)
    };

    let audio_output = AudioOutput {
        _stream: stream,
        _pcm_sink: pcm_sink,
    };

    (speaker, sound_card, Some(audio_output))
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
        "logger" => {
            let serial_logger = Box::new(SerialLogger::new(port - 1)); // port is 1-based
            match port {
                1 => {
                    computer.set_com1_device(serial_logger);
                    log::info!("Serial logger attached to COM1");
                }
                2 => {
                    computer.set_com2_device(serial_logger);
                    log::info!("Serial logger attached to COM2");
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

/// Parse a mount directory argument in the format "/path:E:" or "/path:0x82".
/// Returns the path and the requested drive letter.
pub fn parse_mount_arg(arg: &str) -> Result<(PathBuf, char)> {
    let mut parts: Vec<&str> = arg.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid mount format. Use: /path:E: or /path:0x82"
        ));
    }

    // Handle trailing colon case: "path:D:" splits as ["", "path:D"]
    // We need to re-split to get the drive spec
    if parts[0].is_empty() && parts[1].contains(':') {
        parts = parts[1].rsplitn(2, ':').collect();
    }

    let path = PathBuf::from(parts[1]);
    let drive_spec = parts[0].trim_end_matches(':');

    let drive_letter = if let Some(hex_part) = drive_spec.strip_prefix("0x") {
        // Hex format: 0x82 -> D:
        let num = u8::from_str_radix(hex_part, 16).context("Invalid hex drive number")?;
        DriveNumber::from_standard(num).to_letter()
    } else if drive_spec.len() == 1 {
        // Letter format: E
        drive_spec.chars().next().unwrap().to_ascii_uppercase()
    } else {
        return Err(anyhow::anyhow!("Invalid drive specifier: {}", drive_spec));
    };

    if drive_letter < 'C' {
        return Err(anyhow::anyhow!(
            "Mounted directories must use hard drive letters (C: or higher)"
        ));
    }

    Ok((path, drive_letter))
}

/// Load mounted directories into the computer.
/// Returns the list of mounted drive numbers for later sync.
pub fn load_mounted_directories<V: VideoController>(
    computer: &mut Computer<V>,
    mount_dirs: &[String],
) -> Result<Vec<DriveNumber>> {
    let mut mounted_drives = Vec::new();

    for mount_spec in mount_dirs.iter() {
        let (host_path, drive_letter) = parse_mount_arg(mount_spec)?;

        if !host_path.exists() {
            return Err(anyhow::anyhow!(
                "Mount path does not exist: {}",
                host_path.display()
            ));
        }
        if !host_path.is_dir() {
            return Err(anyhow::anyhow!(
                "Mount path is not a directory: {}",
                host_path.display()
            ));
        }

        log::info!(
            "Mounting {} as drive {}...",
            host_path.display(),
            drive_letter
        );

        let host_disk = HostDirectoryDisk::new(host_path, false)?;
        let drive_num = computer.bios_mut().add_hard_drive(Box::new(host_disk));

        if drive_num.to_letter() != drive_letter {
            log::warn!(
                "Requested drive {} but assigned {} (drives must be sequential)",
                drive_letter,
                drive_num.to_letter()
            );
        }

        mounted_drives.push(drive_num);
        log::info!("Mounted as drive {}", drive_num.to_letter());
    }

    // Update BDA hard drive count after adding mounted drives
    if !mounted_drives.is_empty() {
        computer.update_bda_hard_drive_count();
    }

    Ok(mounted_drives)
}

/// Load CD-ROM ISO images into the computer (slots 0-3).
pub fn load_cdroms<V: VideoController>(
    computer: &mut Computer<V>,
    cdroms: &[String],
) -> Result<()> {
    for (slot, path) in cdroms.iter().enumerate().take(4) {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read CD-ROM image: {}", path))?;
        let image = CdRomImage::new(data)
            .map_err(|e| anyhow::anyhow!("Invalid ISO image {}: {}", path, e))?;
        let drive_num = computer.bios_mut().insert_cdrom(slot as u8, image);
        log::info!(
            "Loaded CD-ROM slot {} (drive {}) from {}",
            slot,
            drive_num,
            path
        );
    }
    Ok(())
}

/// Sync all mounted directories to the host filesystem.
pub fn sync_mounted_directories<V: VideoController>(
    computer: &mut Computer<V>,
    mounted_drives: &[DriveNumber],
) -> Result<()> {
    if mounted_drives.is_empty() {
        return Ok(());
    }

    log::info!("Syncing {} mounted directories...", mounted_drives.len());

    for &drive_num in mounted_drives {
        if let Err(e) = computer.bios_mut().sync_drive(drive_num) {
            log::error!("Failed to sync drive {}: {}", drive_num.to_letter(), e);
        }
    }

    Ok(())
}
