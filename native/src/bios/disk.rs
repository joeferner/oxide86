use emu86_core::cpu::bios::{disk_errors, DriveParams};
use emu86_core::{DiskController, SECTOR_SIZE};

// Disk-related operations for NativeBios

pub fn disk_reset<D: DiskController>(_disk: &mut D, _drive: u8) -> bool {
    // Always succeed for reset
    true
}

pub fn disk_read_sectors<D: DiskController>(
    disk: &mut D,
    _drive: u8,
    cylinder: u8,
    head: u8,
    sector: u8,
    count: u8,
) -> Result<Vec<u8>, u8> {
    let mut result = Vec::with_capacity(count as usize * SECTOR_SIZE);

    for i in 0..count {
        // Calculate CHS for each sector
        let current_sector = sector + i;

        match disk.read_sector_chs(cylinder as u16, head as u16, current_sector as u16) {
            Ok(sector_data) => {
                result.extend_from_slice(&sector_data);
            }
            Err(_) => {
                return Err(disk_errors::SECTOR_NOT_FOUND);
            }
        }
    }

    Ok(result)
}

pub fn disk_write_sectors<D: DiskController>(
    disk: &mut D,
    _drive: u8,
    cylinder: u8,
    head: u8,
    sector: u8,
    count: u8,
    data: &[u8],
) -> Result<u8, u8> {
    if disk.is_read_only() {
        return Err(disk_errors::WRITE_PROTECTED);
    }

    let mut sectors_written = 0;

    for i in 0..count {
        let offset = i as usize * SECTOR_SIZE;
        if offset + SECTOR_SIZE > data.len() {
            break;
        }

        let current_sector = sector + i;
        let mut sector_data = [0u8; SECTOR_SIZE];
        sector_data.copy_from_slice(&data[offset..offset + SECTOR_SIZE]);

        match disk.write_sector_chs(
            cylinder as u16,
            head as u16,
            current_sector as u16,
            &sector_data,
        ) {
            Ok(_) => {
                sectors_written += 1;
            }
            Err(_) => {
                if sectors_written == 0 {
                    return Err(disk_errors::SECTOR_NOT_FOUND);
                } else {
                    return Ok(sectors_written);
                }
            }
        }
    }

    Ok(sectors_written)
}

pub fn disk_get_params<D: DiskController>(disk: &D, _drive: u8) -> Result<DriveParams, u8> {
    let geom = disk.geometry();
    Ok(DriveParams {
        max_cylinder: (geom.cylinders - 1).min(255) as u8,
        max_head: (geom.heads - 1).min(255) as u8,
        max_sector: geom.sectors_per_track.min(63) as u8,
        drive_count: 1,
    })
}

pub fn disk_get_type<D: DiskController>(disk: &D, drive: u8) -> Result<(u8, u32), u8> {
    let geom = disk.geometry();

    // Determine drive type based on drive number
    // 0x00-0x7F are floppy drives, 0x80-0xFF are hard disks
    let drive_type = if drive < 0x80 {
        // Floppy disk with change-line support
        0x02
    } else {
        // Fixed disk (hard disk)
        0x03
    };

    // Calculate total sector count
    let total_sectors =
        geom.cylinders as u32 * geom.heads as u32 * geom.sectors_per_track as u32;

    Ok((drive_type, total_sectors))
}
