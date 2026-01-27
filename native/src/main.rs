use anyhow::{Context, Result};
use clap::Parser;
use emu86_core::{BackedDisk, Computer, MaybePartitionedDisk};
use std::fs::File;
use std::time::{Duration, Instant};

mod bios;
use bios::NativeBios;

mod disk_backend;
use disk_backend::FileDiskBackend;

mod simple_io_device;
use simple_io_device::SimpleIoDevice;

mod terminal_video;
use terminal_video::TerminalVideo;

#[derive(Parser)]
#[command(name = "emu86")]
#[command(about = "Intel 8086 CPU Emulator", long_about = None)]
struct Cli {
    /// Path to the program binary to load and execute (not used with --boot)
    #[arg(required_unless_present = "boot")]
    program: Option<String>,

    /// Boot from disk image instead of loading a program
    #[arg(long)]
    boot: bool,

    /// Boot drive number (0x00 for floppy A:, 0x01 for floppy B:, 0x80 for hard disk C:)
    #[arg(long, default_value = "0x00")]
    boot_drive: String,

    /// Starting segment address (default: 0x0000)
    #[arg(long, default_value = "0x0000")]
    segment: String,

    /// Starting offset address (default: 0x0100, like .COM files)
    #[arg(long, default_value = "0x0100")]
    offset: String,

    /// Enable verbose I/O port logging
    #[arg(long)]
    verbose_io: bool,

    /// Path to disk image file for floppy A:
    #[arg(long = "floppy-a")]
    floppy_a: Option<String>,

    /// Path to disk image file for floppy B:
    #[arg(long = "floppy-b")]
    floppy_b: Option<String>,

    /// Path to hard disk image file(s) - can be specified multiple times for C:, D:, etc.
    #[arg(long = "hdd", action = clap::ArgAction::Append)]
    hard_disks: Vec<String>,

    /// CPU clock speed in MHz (default: 4.77 for original 8086)
    #[arg(long, default_value = "4.77")]
    speed: f64,

    /// Run at maximum speed (no throttling)
    #[arg(long)]
    turbo: bool,
}

fn main() -> Result<()> {
    let log_file = File::create("/tmp/emu86.log").context("Failed to create log file")?;
    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    let cli = Cli::parse();

    // Create BIOS with no drives attached
    let mut bios: NativeBios<MaybePartitionedDisk<FileDiskBackend>> = NativeBios::new();

    // Load floppy A:
    if let Some(path) = &cli.floppy_a {
        let backend = FileDiskBackend::open(path, false)?;
        let disk = BackedDisk::new(backend)
            .with_context(|| format!("Failed to create disk from: {}", path))?;
        bios.insert_floppy(0, MaybePartitionedDisk::Raw(disk))
            .map_err(|e| anyhow::anyhow!("Failed to insert floppy A:: {}", e))?;
        log::info!("Opened floppy A: from {}", path);
    }

    // Load floppy B:
    if let Some(path) = &cli.floppy_b {
        let backend = FileDiskBackend::open(path, false)?;
        let disk = BackedDisk::new(backend)
            .with_context(|| format!("Failed to create disk from: {}", path))?;
        bios.insert_floppy(1, MaybePartitionedDisk::Raw(disk))
            .map_err(|e| anyhow::anyhow!("Failed to insert floppy B:: {}", e))?;
        log::info!("Opened floppy B: from {}", path);
    }

    // Load hard drives (C:, D:, etc.)
    for (i, path) in cli.hard_disks.iter().enumerate() {
        let backend = FileDiskBackend::open(path, false)?;
        let disk = BackedDisk::new(backend)
            .with_context(|| format!("Failed to create disk from: {}", path))?;

        // Check if disk has MBR and partitions
        use emu86_core::{DiskController, parse_mbr};
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
            use emu86_core::PartitionedDisk;
            let partitioned =
                PartitionedDisk::new(disk, partition.start_sector, partition.sector_count);
            bios.add_hard_drive(MaybePartitionedDisk::Partitioned(partitioned))
        } else {
            log::info!("No MBR detected on {}, using raw disk", path);
            bios.add_hard_drive(MaybePartitionedDisk::Raw(disk))
        };

        let drive_letter = (b'C' + i as u8) as char;
        log::info!(
            "Opened hard drive {}: (0x{:02X}) from {}",
            drive_letter,
            drive_num,
            path
        );
    }

    // If no drives specified and booting, error out
    if cli.floppy_a.is_none() && cli.floppy_b.is_none() && cli.hard_disks.is_empty() && cli.boot {
        return Err(anyhow::anyhow!(
            "No disk images specified. Use --floppy-a, --floppy-b, or --hdd to specify disk images."
        ));
    }

    let io_device = SimpleIoDevice::new(cli.verbose_io);
    let video = TerminalVideo::new();
    let mut computer = Computer::new(bios, io_device, video);

    if cli.boot {
        // Boot from disk
        let boot_drive = parse_hex_or_dec(&cli.boot_drive)?;
        if boot_drive > 0xFF {
            return Err(anyhow::anyhow!(
                "Boot drive must be 0x00-0xFF, got 0x{:04X}",
                boot_drive
            ));
        }

        log::info!("Booting from drive 0x{:02X}...", boot_drive);
        computer
            .boot(boot_drive as u8)
            .context("Failed to boot from disk")?;

        log::info!("Boot sector loaded at 0x0000:0x7C00");
        log::info!("Starting execution...\n");
    } else {
        // Load program from file
        let program_path = cli.program.as_ref().unwrap(); // Safe because of required_unless_present
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
        log::info!("Starting execution...\n");
    }

    // Run the program with optional speed throttling
    if cli.turbo {
        log::info!("Running in turbo mode (no speed limit)");
        computer.run();
    } else {
        let clock_hz = (cli.speed * 1_000_000.0) as u64;
        log::info!("Running at {:.2} MHz ({} Hz)", cli.speed, clock_hz);
        run_throttled(&mut computer, clock_hz);
    }

    log::info!("\n=== Execution complete ===");
    computer.dump_registers();

    Ok(())
}

/// Run the emulator with clock speed throttling
fn run_throttled<B, I, V>(computer: &mut Computer<B, I, V>, clock_hz: u64)
where
    B: emu86_core::Bios,
    I: emu86_core::IoDevice,
    V: emu86_core::VideoController,
{
    let start_time = Instant::now();
    let nanos_per_cycle = 1_000_000_000u64 / clock_hz;

    // Run instructions in batches to reduce timing overhead
    const BATCH_SIZE: u32 = 1000;

    while !computer.is_halted() {
        // Execute a batch of instructions
        for _ in 0..BATCH_SIZE {
            if computer.is_halted() {
                break;
            }
            computer.step();
            computer.update_video();
        }

        // Calculate how much time should have elapsed
        let cycles_executed = computer.get_cycle_count();
        let expected_nanos = cycles_executed * nanos_per_cycle;
        let expected_duration = Duration::from_nanos(expected_nanos);

        // Sleep if we're running ahead of schedule
        let actual_elapsed = start_time.elapsed();
        if actual_elapsed < expected_duration {
            let sleep_duration = expected_duration - actual_elapsed;
            // Only sleep if it's worth it (> 100 microseconds)
            if sleep_duration > Duration::from_micros(100) {
                std::thread::sleep(sleep_duration);
            }
        }
    }
}

fn parse_hex_or_dec(s: &str) -> Result<u16> {
    if let Some(hex) = s.strip_prefix("0x") {
        u16::from_str_radix(hex, 16).with_context(|| format!("Invalid hex value: {}", s))
    } else {
        s.parse::<u16>()
            .with_context(|| format!("Invalid decimal value: {}", s))
    }
}
