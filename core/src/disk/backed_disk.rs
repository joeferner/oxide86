use crate::disk::{Disk, DiskBackend, DiskError, DiskGeometry, disk_geometry::SECTOR_SIZE};
use anyhow::{Result, anyhow};
use std::cell::RefCell;

/// Disk backed by a DiskBackend (file, memory, callbacks, etc.)
/// This allows direct I/O to disk image files without loading into memory.
pub struct BackedDisk<B: DiskBackend> {
    backend: RefCell<B>,
    geometry: DiskGeometry,
    read_only: bool,
}

impl<B: DiskBackend> BackedDisk<B> {
    /// Create a new backed disk with auto-detected geometry
    pub fn new(backend: B) -> Result<Self> {
        let size = backend.size();
        let geometry = DiskGeometry::from_size(size as usize)
            .ok_or_else(|| anyhow!("Unsupported disk image size: {} bytes", size))?;
        Ok(Self {
            backend: RefCell::new(backend),
            geometry,
            read_only: false,
        })
    }

    /// Create a new backed disk with specific geometry
    pub fn with_geometry(backend: B, geometry: DiskGeometry) -> Result<Self> {
        let size = backend.size();
        if size as usize != geometry.total_size {
            return Err(anyhow!(
                "Backend size ({}) doesn't match geometry size ({})",
                size,
                geometry.total_size
            ));
        }
        Ok(Self {
            backend: RefCell::new(backend),
            geometry,
            read_only: false,
        })
    }

    /// Set read-only flag
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    /// Get a reference to the underlying backend
    pub fn backend(&self) -> std::cell::Ref<'_, B> {
        self.backend.borrow()
    }

    /// Flush any pending writes to storage
    pub fn flush(&self) -> Result<()> {
        self.backend.borrow_mut().flush()
    }

    /// Read a sector at the given CHS address
    fn read_sector_chs(&self, cylinder: u16, head: u16, sector: u16) -> Result<[u8; SECTOR_SIZE]> {
        let lba = self.geometry.chs_to_lba(cylinder, head, sector)?;
        self.read_sector_lba(lba)
    }

    fn write_sector_chs(&self, cylinder: u16, head: u16, sector: u16, data: &[u8]) -> Result<()> {
        let lba = self.geometry.chs_to_lba(cylinder, head, sector)?;
        let offset = (lba * SECTOR_SIZE) as u64;
        self.backend.borrow_mut().write_at(offset, data)?;
        Ok(())
    }

    fn read_sector_lba(&self, lba: usize) -> Result<[u8; SECTOR_SIZE]> {
        if lba >= self.geometry.total_sectors() {
            return Err(anyhow!(
                "Invalid LBA: {} (max: {})",
                lba,
                self.geometry.total_sectors() - 1
            ));
        }

        let offset = (lba * SECTOR_SIZE) as u64;
        let mut sector = [0u8; SECTOR_SIZE];
        self.backend.borrow_mut().read_at(offset, &mut sector)?;
        Ok(sector)
    }
}

impl<B: DiskBackend> Disk for BackedDisk<B> {
    fn read_sectors(
        &self,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, super::DiskError> {
        // Get disk geometry for proper C/H/S wrapping
        let sectors_per_track = self.geometry.sectors_per_track;
        let heads = self.geometry.heads;

        let mut current_cylinder = cylinder as u16;
        let mut current_head = head as u16;
        let mut current_sector = sector as u16;

        let mut result = Vec::new();

        for _ in 0..count {
            let sector_data = self
                .read_sector_chs(current_cylinder, current_head, current_sector)
                .map_err(|_| DiskError::SectorNotFound)?;
            result.extend_from_slice(&sector_data);

            // Advance to next sector with proper C/H/S wrapping
            current_sector += 1;
            if current_sector > sectors_per_track {
                current_sector = 1;
                current_head += 1;
                if current_head >= heads {
                    current_head = 0;
                    current_cylinder += 1;
                }
            }
        }

        Ok(result)
    }

    fn write_sectors(
        &self,
        cylinder: u8,
        head: u8,
        sector: u8,
        data: &[u8],
    ) -> Result<(), super::DiskError> {
        if self.read_only {
            return Err(super::DiskError::WriteProtected);
        }
        let sectors_per_track = self.geometry.sectors_per_track;
        let heads = self.geometry.heads;

        let mut current_cylinder = cylinder as u16;
        let mut current_head = head as u16;
        let mut current_sector = sector as u16;

        let count = data.len() / SECTOR_SIZE;
        for i in 0..count {
            let sector_data = &data[i * SECTOR_SIZE..(i + 1) * SECTOR_SIZE];
            self.write_sector_chs(current_cylinder, current_head, current_sector, sector_data)
                .map_err(|_| DiskError::SectorNotFound)?;

            current_sector += 1;
            if current_sector > sectors_per_track {
                current_sector = 1;
                current_head += 1;
                if current_head >= heads {
                    current_head = 0;
                    current_cylinder += 1;
                }
            }
        }
        Ok(())
    }

    fn disk_geometry(&self) -> DiskGeometry {
        self.geometry
    }
}
