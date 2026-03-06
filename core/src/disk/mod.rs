use anyhow::Result;

mod backed_disk;
mod disk_error;
mod disk_geometry;
mod drive_number;
mod mem_backend;

pub use backed_disk::BackedDisk;
pub(crate) use disk_error::DiskError;
pub use disk_geometry::DiskGeometry;
pub use drive_number::DriveNumber;
pub use mem_backend::MemBackend;

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

    fn write_sectors(
        &self,
        cylinder: u8,
        head: u8,
        sector: u8,
        data: &[u8],
    ) -> Result<(), DiskError>;

    fn disk_geometry(&self) -> DiskGeometry;
}

