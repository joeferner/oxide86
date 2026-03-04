use anyhow::Result;

mod backed_disk;
mod disk_error;
mod disk_geometry;
mod drive_number;
mod floppy_disk_controller;
mod mem_backend;

pub use backed_disk::BackedDisk;
pub use disk_error::DiskError;
pub use disk_geometry::DiskGeometry;
pub use drive_number::DriveNumber;
pub use floppy_disk_controller::{
    FDC_DATA, FDC_DIR, FDC_DIR_DISK_CHANGE, FDC_DOR, FDC_MSR, FDC_MSR_CB, FDC_MSR_NDM, FDC_MSR_RQM,
    FloppyDiskController,
};
pub use mem_backend::MemBackend;

use crate::bus::Bus;

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
    if drive.is_floppy() {
        bus.floppy_controller()
            .read_sectors(drive, cylinder, head, sector, count)
    } else {
        log::warn!("disk_read_sectors: hard drive support not yet implemented");
        Err(DiskError::DriveNotReady)
    }
}
