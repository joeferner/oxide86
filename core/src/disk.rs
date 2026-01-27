use anyhow::{Result, anyhow};
use std::cell::RefCell;

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
        matches!(self.total_size, 1_474_560 | 737_280 | 368_640)
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

    /// Flush any pending writes to storage
    pub fn flush(&self) -> Result<()> {
        self.backend.borrow_mut().flush()
    }
}

impl<B: DiskBackend> DiskController for BackedDisk<B> {
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

        let offset = (lba * SECTOR_SIZE) as u64;
        let mut sector = [0u8; SECTOR_SIZE];
        self.backend.borrow_mut().read_at(offset, &mut sector)?;
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

        let offset = (lba * SECTOR_SIZE) as u64;
        self.backend.borrow_mut().write_at(offset, data)?;
        Ok(())
    }

    fn geometry(&self) -> &DiskGeometry {
        &self.geometry
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }
}

/// MBR Partition Table Entry (16 bytes)
#[derive(Debug, Clone, Copy)]
pub struct PartitionEntry {
    /// Boot indicator (0x80 = bootable, 0x00 = non-bootable)
    pub bootable: u8,
    /// Partition type (e.g., 0x01 = FAT12, 0x04/0x06 = FAT16, 0x0B/0x0C = FAT32)
    pub partition_type: u8,
    /// Starting sector (LBA)
    pub start_sector: u32,
    /// Size in sectors
    pub sector_count: u32,
}

impl PartitionEntry {
    /// Parse a partition entry from 16 bytes at the given offset
    fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 16 {
            return None;
        }

        let bootable = data[0];
        let partition_type = data[4];
        let start_sector = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let sector_count = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);

        // Empty partition entry
        if partition_type == 0 {
            return None;
        }

        Some(Self {
            bootable,
            partition_type,
            start_sector,
            sector_count,
        })
    }
}

/// Parse MBR partition table from sector 0
/// Returns up to 4 partition entries
pub fn parse_mbr(sector_0: &[u8; SECTOR_SIZE]) -> Option<[Option<PartitionEntry>; 4]> {
    // Check MBR signature (0x55AA at bytes 510-511)
    if sector_0[510] != 0x55 || sector_0[511] != 0xAA {
        return None;
    }

    // Partition table starts at offset 0x1BE (446)
    let mut partitions = [None; 4];
    for (i, partition) in partitions.iter_mut().enumerate() {
        let offset = 0x1BE + i * 16;
        *partition = PartitionEntry::from_bytes(&sector_0[offset..offset + 16]);
    }

    Some(partitions)
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

/// Wrapper around a disk that provides access to a single partition
/// All sector accesses are offset by the partition's start sector
#[derive(Debug)]
pub struct PartitionedDisk<D: DiskController> {
    disk: D,
    partition_start: usize,
    partition_sectors: usize,
    geometry: DiskGeometry,
}

impl<D: DiskController> PartitionedDisk<D> {
    /// Create a new partitioned disk wrapper
    /// partition_start: LBA of first sector in partition
    /// partition_sectors: Number of sectors in partition
    pub fn new(disk: D, partition_start: u32, partition_sectors: u32) -> Self {
        // Calculate geometry for the partition
        let geometry = DiskGeometry::hard_drive(partition_sectors as usize);

        log::info!(
            "PartitionedDisk: Created partition view starting at sector {} with {} sectors",
            partition_start,
            partition_sectors
        );

        Self {
            disk,
            partition_start: partition_start as usize,
            partition_sectors: partition_sectors as usize,
            geometry,
        }
    }

    /// Get the underlying disk (for boot operations that need raw access)
    pub fn into_inner(self) -> D {
        self.disk
    }
}

impl<D: DiskController> DiskController for PartitionedDisk<D> {
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
        if lba >= self.partition_sectors {
            return Err(anyhow!(
                "LBA {} exceeds partition size ({})",
                lba,
                self.partition_sectors
            ));
        }

        // Offset LBA by partition start
        let absolute_lba = self.partition_start + lba;
        log::debug!(
            "PartitionedDisk::read_sector_lba: partition LBA {} → absolute LBA {} (partition starts at {})",
            lba,
            absolute_lba,
            self.partition_start
        );
        self.disk.read_sector_lba(absolute_lba)
    }

    fn write_sector_lba(&mut self, lba: usize, data: &[u8; SECTOR_SIZE]) -> Result<()> {
        if lba >= self.partition_sectors {
            return Err(anyhow!(
                "LBA {} exceeds partition size ({})",
                lba,
                self.partition_sectors
            ));
        }

        // Offset LBA by partition start
        let absolute_lba = self.partition_start + lba;
        self.disk.write_sector_lba(absolute_lba, data)
    }

    fn geometry(&self) -> &DiskGeometry {
        &self.geometry
    }

    fn size(&self) -> usize {
        self.partition_sectors * SECTOR_SIZE
    }

    fn is_read_only(&self) -> bool {
        self.disk.is_read_only()
    }
}

/// Enum to represent either a raw disk or a partitioned disk
pub enum MaybePartitionedDisk<B: DiskBackend> {
    Raw(BackedDisk<B>),
    Partitioned(PartitionedDisk<BackedDisk<B>>),
}

impl<B: DiskBackend> DiskController for MaybePartitionedDisk<B> {
    fn read_sector_chs(
        &self,
        cylinder: u16,
        head: u16,
        sector: u16,
    ) -> anyhow::Result<[u8; SECTOR_SIZE]> {
        match self {
            Self::Raw(disk) => disk.read_sector_chs(cylinder, head, sector),
            Self::Partitioned(disk) => disk.read_sector_chs(cylinder, head, sector),
        }
    }

    fn write_sector_chs(
        &mut self,
        cylinder: u16,
        head: u16,
        sector: u16,
        data: &[u8; SECTOR_SIZE],
    ) -> anyhow::Result<()> {
        match self {
            Self::Raw(disk) => disk.write_sector_chs(cylinder, head, sector, data),
            Self::Partitioned(disk) => disk.write_sector_chs(cylinder, head, sector, data),
        }
    }

    fn read_sector_lba(&self, lba: usize) -> anyhow::Result<[u8; SECTOR_SIZE]> {
        match self {
            Self::Raw(disk) => disk.read_sector_lba(lba),
            Self::Partitioned(disk) => disk.read_sector_lba(lba),
        }
    }

    fn write_sector_lba(&mut self, lba: usize, data: &[u8; SECTOR_SIZE]) -> anyhow::Result<()> {
        match self {
            Self::Raw(disk) => disk.write_sector_lba(lba, data),
            Self::Partitioned(disk) => disk.write_sector_lba(lba, data),
        }
    }

    fn geometry(&self) -> &DiskGeometry {
        match self {
            Self::Raw(disk) => disk.geometry(),
            Self::Partitioned(disk) => disk.geometry(),
        }
    }

    fn size(&self) -> usize {
        match self {
            Self::Raw(disk) => disk.size(),
            Self::Partitioned(disk) => disk.size(),
        }
    }

    fn is_read_only(&self) -> bool {
        match self {
            Self::Raw(disk) => disk.is_read_only(),
            Self::Partitioned(disk) => disk.is_read_only(),
        }
    }
}
