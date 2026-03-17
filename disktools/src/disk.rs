use anyhow::{Context, Result};
use oxide86_core::SECTOR_SIZE;
use std::io::{self, Read, Seek, SeekFrom, Write};

/// MBR Partition Table Entry
#[derive(Debug, Clone, Copy)]
pub struct PartitionEntry {
    pub start_sector: u32,
    pub sector_count: u32,
}

impl PartitionEntry {
    fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 16 {
            return None;
        }
        let partition_type = data[4];
        if partition_type == 0 {
            return None;
        }
        let start_sector = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let sector_count = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        Some(Self {
            start_sector,
            sector_count,
        })
    }
}

/// Parse MBR partition table from sector 0.
/// Returns None for non-MBR sectors (including FAT boot records).
pub fn parse_mbr(sector_0: &[u8; SECTOR_SIZE]) -> Option<[Option<PartitionEntry>; 4]> {
    if sector_0[510] != 0x55 || sector_0[511] != 0xAA {
        return None;
    }
    // Guard against FAT VBR false positives: FAT BPB always has a valid bytes_per_sector
    // (power-of-two in {512,1024,2048,4096}) at offset 11-12; real MBR code never does.
    let bytes_per_sector = u16::from_le_bytes([sector_0[11], sector_0[12]]);
    if matches!(bytes_per_sector, 512 | 1024 | 2048 | 4096) {
        return None;
    }
    let mut partitions = [None; 4];
    for (i, entry) in partitions.iter_mut().enumerate() {
        let offset = 0x1BE + i * 16;
        *entry = PartitionEntry::from_bytes(&sector_0[offset..offset + 16]);
    }
    Some(partitions)
}

/// A cursor over a Vec<u8> that is confined to a byte range [start, start+len).
/// Used to expose a FAT partition within a full disk image to fatfs.
pub struct PartitionCursor {
    data: Vec<u8>,
    /// Byte offset into `data` where the partition starts
    start: usize,
    /// Byte length of the partition region
    len: usize,
    /// Current position relative to `start`
    pos: usize,
}

impl PartitionCursor {
    pub fn new(data: Vec<u8>, start: usize, len: usize) -> Self {
        Self {
            data,
            start,
            len,
            pos: 0,
        }
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

impl Read for PartitionCursor {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let remaining = self.len.saturating_sub(self.pos);
        let to_read = buf.len().min(remaining);
        let abs = self.start + self.pos;
        buf[..to_read].copy_from_slice(&self.data[abs..abs + to_read]);
        self.pos += to_read;
        Ok(to_read)
    }
}

impl Write for PartitionCursor {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let remaining = self.len.saturating_sub(self.pos);
        let to_write = buf.len().min(remaining);
        let abs = self.start + self.pos;
        self.data[abs..abs + to_write].copy_from_slice(&buf[..to_write]);
        self.pos += to_write;
        Ok(to_write)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for PartitionCursor {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::End(n) => self.len as i64 + n,
            SeekFrom::Current(n) => self.pos as i64 + n,
        };
        if new_pos < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before start",
            ));
        }
        self.pos = new_pos as usize;
        Ok(self.pos as u64)
    }
}

/// Detect whether the disk has an MBR partition table and return
/// (partition_start_bytes, partition_len_bytes). Falls back to (0, data.len())
/// for unpartitioned (floppy-style) images.
pub fn partition_bounds(data: &[u8]) -> (usize, usize) {
    if data.len() < SECTOR_SIZE {
        return (0, data.len());
    }
    let sector0: &[u8; SECTOR_SIZE] = data[..SECTOR_SIZE].try_into().unwrap();
    if let Some(partitions) = parse_mbr(sector0)
        && let Some(p) = partitions.iter().flatten().next()
    {
        let start = p.start_sector as usize * SECTOR_SIZE;
        let len = p.sector_count as usize * SECTOR_SIZE;
        if start + len <= data.len() {
            return (start, len);
        }
    }
    (0, data.len())
}

/// Load a disk image from `path` and return a PartitionCursor ready for fatfs.
pub fn open_disk(path: &str) -> Result<PartitionCursor> {
    let data = std::fs::read(path).with_context(|| format!("reading disk image '{path}'"))?;
    let (start, len) = partition_bounds(&data);
    Ok(PartitionCursor::new(data, start, len))
}

/// Open the disk, call `f` with the fatfs filesystem, then write the modified
/// image back to `path`. Use this for write operations.
pub fn with_disk_mut<F>(path: &str, f: F) -> Result<()>
where
    F: FnOnce(&mut fatfs::FileSystem<&mut PartitionCursor>) -> Result<()>,
{
    let data = std::fs::read(path).with_context(|| format!("reading disk image '{path}'"))?;
    let (start, len) = partition_bounds(&data);
    let mut cursor = PartitionCursor::new(data, start, len);
    {
        let mut fs = fatfs::FileSystem::new(&mut cursor, fatfs::FsOptions::new())
            .with_context(|| format!("opening FAT filesystem in '{path}'"))?;
        f(&mut fs)?;
        fs.unmount()
            .with_context(|| format!("unmounting filesystem in '{path}'"))?;
    }
    std::fs::write(path, cursor.into_data())
        .with_context(|| format!("writing disk image '{path}'"))?;
    Ok(())
}

/// Normalise a disk path: trim leading `::`, convert backslashes, ensure leading `/`.
pub fn normalise_disk_path(raw: &str) -> String {
    let stripped = raw.strip_prefix("::").unwrap_or(raw);
    let forward = stripped.replace('\\', "/");
    if forward.starts_with('/') {
        forward
    } else {
        format!("/{forward}")
    }
}

/// Returns true if `s` is a disk path (starts with `::`)
pub fn is_disk_path(s: &str) -> bool {
    s.starts_with("::")
}
