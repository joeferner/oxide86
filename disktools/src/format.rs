use anyhow::Result;
use clap::{ArgGroup, Args};
use oxide86_core::{DiskGeometry, create_formatted_disk};
use std::fs;

#[derive(Args)]
#[command(group(ArgGroup::new("disk_type").required(true)))]
pub struct FormatArgs {
    /// Output file path
    output: String,

    /// Create 1.44MB 3.5" HD floppy image (FAT12)
    #[arg(long, group = "disk_type")]
    floppy_1440: bool,

    /// Create 720KB 3.5" DD floppy image (FAT12)
    #[arg(long, group = "disk_type")]
    floppy_720: bool,

    /// Create 360KB 5.25" DD floppy image (FAT12)
    #[arg(long, group = "disk_type")]
    floppy_360: bool,

    /// Create 160KB 5.25" SS/SD floppy image (FAT12)
    #[arg(long, group = "disk_type")]
    floppy_160: bool,

    /// Create hard drive image of given size in MB (min 2MB, adds MBR partition table)
    #[arg(long, value_name = "MB", group = "disk_type")]
    hdd: Option<u32>,

    /// Volume label (up to 11 characters)
    #[arg(long, value_name = "LABEL")]
    label: Option<String>,
}

pub fn run(args: FormatArgs) -> Result<()> {
    let geometry = if args.floppy_1440 {
        DiskGeometry::FLOPPY_1440K
    } else if args.floppy_720 {
        DiskGeometry::FLOPPY_720K
    } else if args.floppy_360 {
        DiskGeometry::FLOPPY_360K
    } else if args.floppy_160 {
        DiskGeometry::FLOPPY_160K
    } else if let Some(mb) = args.hdd {
        if mb < 2 {
            anyhow::bail!("HDD size must be at least 2MB");
        }
        let total_sectors = (mb as usize * 1024 * 1024) / 512;
        DiskGeometry::hard_drive(total_sectors)
    } else {
        unreachable!()
    };

    let data = create_formatted_disk(geometry, args.label.as_deref())?;
    let size = data.len();
    fs::write(&args.output, &data)?;

    let kind = if geometry.is_floppy() {
        "floppy (FAT12)".to_string()
    } else {
        "hard drive with MBR partition (FAT16/FAT32)".to_string()
    };
    println!("Created: {} ({} bytes, {})", args.output, size, kind);
    Ok(())
}
