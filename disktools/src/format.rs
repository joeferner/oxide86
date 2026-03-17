use anyhow::{Result, anyhow};
use clap::{ArgGroup, Args};
use oxide86_core::DiskGeometry;
use std::fs;
use std::io::Cursor;

use oxide86_core::SECTOR_SIZE;

fn build_format_opts(geometry: &DiskGeometry, label: Option<&str>) -> fatfs::FormatVolumeOptions {
    let mut opts = fatfs::FormatVolumeOptions::new();
    opts = opts
        .heads(geometry.heads)
        .sectors_per_track(geometry.sectors_per_track);
    match geometry.total_size {
        1_474_560 => {
            opts = opts
                .media(0xF0)
                .bytes_per_cluster(512)
                .max_root_dir_entries(224)
                .fat_type(fatfs::FatType::Fat12)
                .drive_num(0);
        }
        737_280 => {
            opts = opts
                .media(0xF9)
                .bytes_per_cluster(1024)
                .max_root_dir_entries(112)
                .fat_type(fatfs::FatType::Fat12)
                .drive_num(0);
        }
        368_640 => {
            opts = opts
                .media(0xFD)
                .bytes_per_cluster(1024)
                .max_root_dir_entries(112)
                .fat_type(fatfs::FatType::Fat12)
                .drive_num(0);
        }
        163_840 => {
            opts = opts
                .media(0xFE)
                .bytes_per_cluster(512)
                .max_root_dir_entries(64)
                .fat_type(fatfs::FatType::Fat12)
                .drive_num(0);
        }
        _ => {}
    }
    if let Some(l) = label {
        let mut lb = [b' '; 11];
        let bytes = l.as_bytes();
        let len = bytes.len().min(11);
        lb[..len].copy_from_slice(&bytes[..len]);
        opts = opts.volume_label(lb);
    }
    opts
}

fn write_mbr_partition(disk: &mut [u8], start_sector: u32, sector_count: u32) {
    const PART_OFFSET: usize = 446;
    disk[PART_OFFSET] = 0x80;
    disk[PART_OFFSET + 1] = 0xFE;
    disk[PART_OFFSET + 2] = 0xFF;
    disk[PART_OFFSET + 3] = 0xFF;
    disk[PART_OFFSET + 4] = if (sector_count as u64 * SECTOR_SIZE as u64) < 32 * 1024 * 1024 {
        0x06
    } else {
        0x0B
    };
    disk[PART_OFFSET + 5] = 0xFE;
    disk[PART_OFFSET + 6] = 0xFF;
    disk[PART_OFFSET + 7] = 0xFF;
    disk[PART_OFFSET + 8..PART_OFFSET + 12].copy_from_slice(&start_sector.to_le_bytes());
    disk[PART_OFFSET + 12..PART_OFFSET + 16].copy_from_slice(&sector_count.to_le_bytes());
    disk[510] = 0x55;
    disk[511] = 0xAA;
}

fn create_formatted_disk(geometry: DiskGeometry, label: Option<&str>) -> Result<Vec<u8>> {
    let opts = build_format_opts(&geometry, label);
    if geometry.is_floppy() {
        let data = vec![0u8; geometry.total_size];
        let mut cursor = Cursor::new(data);
        fatfs::format_volume(&mut cursor, opts).map_err(|e| anyhow!("Format failed: {}", e))?;
        Ok(cursor.into_inner())
    } else {
        const PARTITION_START: usize = 63;
        let total_sectors = geometry.total_sectors();
        let partition_sectors = total_sectors - PARTITION_START;
        let mut disk_data = vec![0u8; geometry.total_size];
        write_mbr_partition(
            &mut disk_data,
            PARTITION_START as u32,
            partition_sectors as u32,
        );
        let partition_size = partition_sectors * SECTOR_SIZE;
        let partition_data = vec![0u8; partition_size];
        let mut cursor = Cursor::new(partition_data);
        fatfs::format_volume(&mut cursor, opts)
            .map_err(|e| anyhow!("Partition format failed: {}", e))?;
        let formatted = cursor.into_inner();
        let offset = PARTITION_START * SECTOR_SIZE;
        disk_data[offset..offset + partition_size].copy_from_slice(&formatted);
        Ok(disk_data)
    }
}

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
