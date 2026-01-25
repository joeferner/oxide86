use anyhow::{Result, anyhow};

/// Standard sector size for floppy disks (512 bytes)
pub const SECTOR_SIZE: usize = 512;

/// Floppy disk geometry specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskGeometry {
    /// Number of cylinders (tracks)
    pub cylinders: u16,
    /// Number of heads (sides)
    pub heads: u16,
    /// Sectors per track
    pub sectors_per_track: u16,
    /// Total size in bytes
    pub total_size: usize,
}

impl DiskGeometry {
    /// 3.5" HD floppy: 1.44 MB (80 tracks, 2 heads, 18 sectors/track)
    pub const FLOPPY_1440K: Self = Self {
        cylinders: 80,
        heads: 2,
        sectors_per_track: 18,
        total_size: 1_474_560, // 80 * 2 * 18 * 512
    };

    /// 3.5" DD floppy: 720 KB (80 tracks, 2 heads, 9 sectors/track)
    pub const FLOPPY_720K: Self = Self {
        cylinders: 80,
        heads: 2,
        sectors_per_track: 9,
        total_size: 737_280, // 80 * 2 * 9 * 512
    };

    /// 5.25" DD floppy: 360 KB (40 tracks, 2 heads, 9 sectors/track)
    pub const FLOPPY_360K: Self = Self {
        cylinders: 40,
        heads: 2,
        sectors_per_track: 9,
        total_size: 368_640, // 40 * 2 * 9 * 512
    };

    /// Detect geometry based on disk image size
    pub fn from_size(size: usize) -> Option<Self> {
        match size {
            1_474_560 => Some(Self::FLOPPY_1440K),
            737_280 => Some(Self::FLOPPY_720K),
            368_640 => Some(Self::FLOPPY_360K),
            _ => None,
        }
    }

    /// Calculate total number of sectors
    pub fn total_sectors(&self) -> usize {
        self.cylinders as usize * self.heads as usize * self.sectors_per_track as usize
    }

    /// Convert CHS (Cylinder/Head/Sector) to LBA (Logical Block Address)
    /// Note: Sectors are 1-indexed in CHS addressing
    pub fn chs_to_lba(&self, cylinder: u16, head: u16, sector: u16) -> Result<usize> {
        if cylinder >= self.cylinders {
            return Err(anyhow!(
                "Invalid cylinder: {} (max: {})",
                cylinder,
                self.cylinders - 1
            ));
        }
        if head >= self.heads {
            return Err(anyhow!("Invalid head: {} (max: {})", head, self.heads - 1));
        }
        if sector == 0 || sector > self.sectors_per_track {
            return Err(anyhow!(
                "Invalid sector: {} (valid range: 1-{})",
                sector,
                self.sectors_per_track
            ));
        }

        // LBA = (C × HPC + H) × SPT + (S − 1)
        // where HPC = heads per cylinder, SPT = sectors per track
        let lba = (cylinder as usize * self.heads as usize + head as usize)
            * self.sectors_per_track as usize
            + (sector as usize - 1);

        Ok(lba)
    }

    /// Convert LBA (Logical Block Address) to CHS (Cylinder/Head/Sector)
    /// Note: Returns sector as 1-indexed
    pub fn lba_to_chs(&self, lba: usize) -> Result<(u16, u16, u16)> {
        if lba >= self.total_sectors() {
            return Err(anyhow!(
                "Invalid LBA: {} (max: {})",
                lba,
                self.total_sectors() - 1
            ));
        }

        let cylinder = lba / (self.heads as usize * self.sectors_per_track as usize);
        let temp = lba % (self.heads as usize * self.sectors_per_track as usize);
        let head = temp / self.sectors_per_track as usize;
        let sector = (temp % self.sectors_per_track as usize) + 1; // 1-indexed

        Ok((cylinder as u16, head as u16, sector as u16))
    }
}

/// Disk controller trait for reading and writing sectors
pub trait DiskController {
    /// Read a sector at the given CHS address
    fn read_sector_chs(&self, cylinder: u16, head: u16, sector: u16) -> Result<[u8; SECTOR_SIZE]>;

    /// Write a sector at the given CHS address
    fn write_sector_chs(
        &mut self,
        cylinder: u16,
        head: u16,
        sector: u16,
        data: &[u8; SECTOR_SIZE],
    ) -> Result<()>;

    /// Read a sector at the given LBA
    fn read_sector_lba(&self, lba: usize) -> Result<[u8; SECTOR_SIZE]>;

    /// Write a sector at the given LBA
    fn write_sector_lba(&mut self, lba: usize, data: &[u8; SECTOR_SIZE]) -> Result<()>;

    /// Get the disk geometry
    fn geometry(&self) -> &DiskGeometry;

    /// Get total size in bytes
    fn size(&self) -> usize {
        self.geometry().total_size
    }

    /// Check if disk is read-only
    fn is_read_only(&self) -> bool;
}

/// Raw disk image stored in memory
/// Platform-agnostic - works with both native and WASM
#[derive(Debug, Clone)]
pub struct DiskImage {
    data: Vec<u8>,
    geometry: DiskGeometry,
    read_only: bool,
}

impl DiskImage {
    /// Create a new disk image from raw data
    pub fn new(data: Vec<u8>) -> Result<Self> {
        let geometry = DiskGeometry::from_size(data.len())
            .ok_or_else(|| anyhow!("Unsupported disk image size: {} bytes", data.len()))?;

        Ok(Self {
            data,
            geometry,
            read_only: false,
        })
    }

    /// Create a new disk image with specific geometry
    pub fn with_geometry(data: Vec<u8>, geometry: DiskGeometry) -> Result<Self> {
        if data.len() != geometry.total_size {
            return Err(anyhow!(
                "Data size ({}) doesn't match geometry size ({})",
                data.len(),
                geometry.total_size
            ));
        }

        Ok(Self {
            data,
            geometry,
            read_only: false,
        })
    }

    /// Create an empty disk image with the given geometry
    pub fn empty(geometry: DiskGeometry) -> Self {
        Self {
            data: vec![0; geometry.total_size],
            geometry,
            read_only: false,
        }
    }

    /// Set read-only flag
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    /// Get a reference to the raw disk data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get a mutable reference to the raw disk data
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl DiskController for DiskImage {
    fn read_sector_chs(&self, cylinder: u16, head: u16, sector: u16) -> Result<[u8; SECTOR_SIZE]> {
        let lba = self.geometry.chs_to_lba(cylinder, head, sector)?;
        self.read_sector_lba(lba)
    }

    fn write_sector_chs(
        &mut self,
        cylinder: u16,
        head: u16,
        sector: u16,
        data: &[u8; SECTOR_SIZE],
    ) -> Result<()> {
        let lba = self.geometry.chs_to_lba(cylinder, head, sector)?;
        self.write_sector_lba(lba, data)
    }

    fn read_sector_lba(&self, lba: usize) -> Result<[u8; SECTOR_SIZE]> {
        if lba >= self.geometry.total_sectors() {
            return Err(anyhow!(
                "Invalid LBA: {} (max: {})",
                lba,
                self.geometry.total_sectors() - 1
            ));
        }

        let offset = lba * SECTOR_SIZE;
        let mut sector = [0u8; SECTOR_SIZE];
        sector.copy_from_slice(&self.data[offset..offset + SECTOR_SIZE]);
        Ok(sector)
    }

    fn write_sector_lba(&mut self, lba: usize, data: &[u8; SECTOR_SIZE]) -> Result<()> {
        if self.read_only {
            return Err(anyhow!("Disk is read-only"));
        }

        if lba >= self.geometry.total_sectors() {
            return Err(anyhow!(
                "Invalid LBA: {} (max: {})",
                lba,
                self.geometry.total_sectors() - 1
            ));
        }

        let offset = lba * SECTOR_SIZE;
        self.data[offset..offset + SECTOR_SIZE].copy_from_slice(data);
        Ok(())
    }

    fn geometry(&self) -> &DiskGeometry {
        &self.geometry
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }
}
