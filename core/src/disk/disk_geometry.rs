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

    /// 5.25" SS/SD floppy: 160 KB (40 tracks, 1 head, 8 sectors/track)
    pub const FLOPPY_160K: Self = Self {
        cylinders: 40,
        heads: 1,
        sectors_per_track: 8,
        total_size: 163_840, // 40 * 1 * 8 * 512
    };

    /// Create a hard drive geometry from total sector count
    /// Uses standard CHS parameters: 63 sectors/track, 16 heads
    /// Maximum 1024 cylinders (CHS addressing limit)
    pub fn hard_drive(total_sectors: usize) -> Self {
        let sectors_per_track = 63u16;
        let heads = 16u16;
        let cylinders =
            ((total_sectors / (sectors_per_track as usize * heads as usize)).min(1024)) as u16;
        let total_size = total_sectors * SECTOR_SIZE;
        Self {
            cylinders,
            heads,
            sectors_per_track,
            total_size,
        }
    }

    /// Detect geometry based on disk image size
    /// Returns known floppy geometries for exact matches, or hard drive geometry for larger disks
    pub fn from_size(size: usize) -> Option<Self> {
        match size {
            1_474_560 => Some(Self::FLOPPY_1440K),
            737_280 => Some(Self::FLOPPY_720K),
            368_640 => Some(Self::FLOPPY_360K),
            163_840 => Some(Self::FLOPPY_160K),
            _ if size >= 2_000_000 && size.is_multiple_of(SECTOR_SIZE) => {
                // Treat as hard drive (>= ~2MB and sector-aligned)
                let total_sectors = size / SECTOR_SIZE;
                Some(Self::hard_drive(total_sectors))
            }
            _ => None,
        }
    }

    /// Check if this geometry represents a floppy disk
    pub fn is_floppy(&self) -> bool {
        matches!(self.total_size, 1_474_560 | 737_280 | 368_640 | 163_840)
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
