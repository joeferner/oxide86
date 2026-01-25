use anyhow::{Context, Result};
use clap::Parser;
use emu86_core::{Computer, DiskImage, DiskGeometry};

mod stdio_bios;
use log::info;
use stdio_bios::StdioBios;

mod simple_io_device;
use simple_io_device::SimpleIoDevice;

mod terminal_video;
use terminal_video::TerminalVideo;

#[derive(Parser)]
#[command(name = "emu86")]
#[command(about = "Intel 8086 CPU Emulator", long_about = None)]
struct Cli {
    /// Path to the program binary to load and execute
    program: String,

    /// Starting segment address (default: 0x0000)
    #[arg(long, default_value = "0x0000")]
    segment: String,

    /// Starting offset address (default: 0x0100, like .COM files)
    #[arg(long, default_value = "0x0100")]
    offset: String,

    /// Enable verbose I/O port logging
    #[arg(long)]
    verbose_io: bool,

    /// Path to disk image file (optional, will create empty 1.44MB floppy if not specified)
    #[arg(long)]
    disk: Option<String>,

    /// Working directory for file operations (default: current directory)
    #[arg(long, default_value = ".")]
    workdir: String,
}

fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    // Create working directory if it doesn't exist
    std::fs::create_dir_all(&cli.workdir)
        .with_context(|| format!("Failed to create working directory: {}", cli.workdir))?;

    // Load or create disk image
    let disk = if let Some(disk_path) = &cli.disk {
        let disk_data = std::fs::read(disk_path)
            .with_context(|| format!("Failed to read disk image: {}", disk_path))?;
        DiskImage::new(disk_data)
            .context("Failed to create disk image from file")?
    } else {
        // Create empty 1.44MB floppy disk
        DiskImage::empty(DiskGeometry::FLOPPY_1440K)
    };

    let io_device = SimpleIoDevice::new(cli.verbose_io);
    let bios = StdioBios::new(disk, &cli.workdir);
    let video = TerminalVideo::new();
    let mut computer = Computer::new(bios, io_device, video);

    // Load the program binary
    let program_data = std::fs::read(&cli.program)
        .with_context(|| format!("Failed to read program file: {}", cli.program))?;

    let segment = parse_hex_or_dec(&cli.segment)?;
    let offset = parse_hex_or_dec(&cli.offset)?;

    computer.load_program(&program_data, segment, offset)
        .context("Failed to load program")?;

    info!("Loaded {} bytes at {:04X}:{:04X}", program_data.len(), segment, offset);
    info!("Starting execution...\n");

    // Run the program
    computer.run();

    info!("\n=== Execution complete ===");
    computer.dump_registers();

    Ok(())
}

fn parse_hex_or_dec(s: &str) -> Result<u16> {
    if let Some(hex) = s.strip_prefix("0x") {
        u16::from_str_radix(hex, 16)
            .with_context(|| format!("Invalid hex value: {}", s))
    } else {
        s.parse::<u16>()
            .with_context(|| format!("Invalid decimal value: {}", s))
    }
}
