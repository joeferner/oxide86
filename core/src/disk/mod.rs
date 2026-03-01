use anyhow::Result;

mod backed_disk;
mod disk_controller;
mod disk_error;
mod disk_geometry;
mod drive_number;

pub use backed_disk::BackedDisk;
pub use disk_controller::DiskController;
pub use disk_error::DiskError;
pub use disk_geometry::DiskGeometry;
pub use drive_number::DriveNumber;

use crate::{bus::Bus, cpu::bios::int13_disk_services::DriveParams};

/// Backend trait for disk storage operations.
/// Implemented by platform-specific code (native uses File, WASM uses callbacks).
pub trait DiskBackend {
    /// Read data at the given byte offset
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize>;

    /// Write data at the given byte offset
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize>;

    /// Flush any buffered writes to underlying storage
    fn flush(&mut self) -> Result<()>;

    /// Get total size in bytes
    fn size(&self) -> u64;
}

pub trait Disk {
    fn read_sectors(
        &self,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, DiskError>;

    fn disk_geometry(&self) -> DiskGeometry;
}

pub fn disk_read_sectors(
    bus: &Bus,
    drive: DriveNumber,
    cylinder: u8,
    head: u8,
    sector: u8,
    count: u8,
) -> Result<Vec<u8>, DiskError> {
    if let Some(disk_controller) = bus.find_disk_controller(drive) {
        disk_controller
            .borrow()
            .read_sectors(cylinder, head, sector, count)
    } else {
        Err(DiskError::DriveNotReady)
    }
}

pub fn disk_get_params(bus: &Bus, drive: DriveNumber) -> Result<DriveParams, DiskError> {
    let disk_controller = bus
        .find_disk_controller(drive)
        .ok_or(DiskError::DriveNotReady)?;

    let geometry = disk_controller.borrow().disk_geometry();

    // Count drives of this type (CD-ROM placeholders excluded from hard drive count)
    // TODO
    // let drive_count = if drive.is_floppy() {
    //     self.devices.floppy_drives_with_disk_count()
    // } else {
    //     self.devices.hard_drive_count()
    // };
    let drive_count = 0;

    Ok(DriveParams {
        max_cylinder: (geometry.cylinders - 1).min(255) as u8,
        max_head: (geometry.heads - 1).min(255) as u8,
        max_sector: geometry.sectors_per_track.min(255) as u8,
        drive_count,
    })
}
