//! Host directory mounting as DOS drives
//!
//! Allows mounting a host filesystem directory as a FAT-formatted DOS drive.
//! Files are scanned from the host, a FAT filesystem is generated in memory,
//! and changes are synced back to the host on close/shutdown.

use anyhow::{Context, Result, anyhow};
use oxide86_core::{
    BackedDisk, DiskController, DiskGeometry, MemoryDiskBackend, SECTOR_SIZE, create_formatted_disk,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// Metadata about a file/directory in the host filesystem
struct FileEntry {
    dos_path: String,   // DOS path (e.g., "SUBDIR/FILE.TXT")
    host_path: PathBuf, // Absolute host path
    size: u64,
    is_dir: bool,
}

/// A disk controller that mounts a host directory as a FAT filesystem.
/// Changes made by DOS programs are synced back to the host directory.
pub struct HostDirectoryDisk {
    host_path: PathBuf,
    fat_image: BackedDisk<MemoryDiskBackend>,
    dirty_sectors: HashSet<usize>,
    read_only: bool,
}

impl HostDirectoryDisk {
    /// Create a new host directory disk by scanning the directory and generating a FAT image.
    pub fn new(host_path: PathBuf, read_only: bool) -> Result<Self> {
        if !host_path.exists() {
            return Err(anyhow!("Path does not exist: {}", host_path.display()));
        }
        if !host_path.is_dir() {
            return Err(anyhow!("Path is not a directory: {}", host_path.display()));
        }

        log::info!("Scanning directory: {}", host_path.display());

        // 1. Scan directory to inventory files
        let files = scan_directory(&host_path)?;
        log::info!("Found {} files and directories", files.len());

        // 2. Calculate required disk size
        let total_bytes: u64 = files.iter().filter(|f| !f.is_dir).map(|f| f.size).sum();
        log::info!("Total file size: {} bytes", total_bytes);

        let geometry = calculate_geometry(total_bytes)?;
        log::info!(
            "Using geometry: {} sectors ({} MB)",
            geometry.total_sectors(),
            geometry.total_size / 1024 / 1024
        );

        // 3. Create and format FAT image with MBR (hard drives need partition table for DOS)
        let disk_data = create_formatted_disk(geometry, Some("HOST_DIR"))?;
        let backend = MemoryDiskBackend::new(disk_data);
        let mut disk = BackedDisk::new(backend)?;

        // 4. Populate FAT image with files from host
        let file_count = populate_fat_image(&mut disk, files)?;
        log::info!("Populated FAT image with {} files", file_count);

        Ok(Self {
            host_path,
            fat_image: disk,
            dirty_sectors: HashSet::new(),
            read_only,
        })
    }

    /// Sync changes from the FAT image back to the host directory.
    /// This extracts all files from the FAT image and writes them to the host.
    pub fn sync_to_host(&mut self) -> Result<()> {
        if self.read_only || self.dirty_sectors.is_empty() {
            return Ok(());
        }

        log::info!(
            "Syncing {} dirty sectors to host: {}",
            self.dirty_sectors.len(),
            self.host_path.display()
        );

        // Extract all files from the FAT partition using fatfs
        const PARTITION_START: usize = 63;
        const SECTOR_SIZE: usize = 512;
        let partition_offset = PARTITION_START * SECTOR_SIZE;

        let backend = self.fat_image.backend().clone();
        let disk_data = backend.get_data();
        let partition_data = &disk_data[partition_offset..];
        let mut cursor: std::io::Cursor<Vec<u8>> = std::io::Cursor::new(partition_data.to_vec());

        let fs = fatfs::FileSystem::new(&mut cursor, fatfs::FsOptions::new())
            .context("Failed to mount FAT partition for sync")?;

        sync_directory(&fs.root_dir(), &self.host_path, "")?;

        // Clear dirty sectors after successful sync
        self.dirty_sectors.clear();
        log::info!("Sync complete");

        Ok(())
    }
}

impl DiskController for HostDirectoryDisk {
    fn read_sector_chs(&self, cylinder: u16, head: u16, sector: u16) -> Result<[u8; SECTOR_SIZE]> {
        self.fat_image.read_sector_chs(cylinder, head, sector)
    }

    fn write_sector_chs(
        &mut self,
        cylinder: u16,
        head: u16,
        sector: u16,
        data: &[u8; SECTOR_SIZE],
    ) -> Result<()> {
        let lba = self.geometry().chs_to_lba(cylinder, head, sector)?;
        self.write_sector_lba(lba, data)
    }

    fn read_sector_lba(&self, lba: usize) -> Result<[u8; SECTOR_SIZE]> {
        self.fat_image.read_sector_lba(lba)
    }

    fn write_sector_lba(&mut self, lba: usize, data: &[u8; SECTOR_SIZE]) -> Result<()> {
        self.fat_image.write_sector_lba(lba, data)?;
        if !self.read_only {
            self.dirty_sectors.insert(lba);
        }
        Ok(())
    }

    fn geometry(&self) -> &DiskGeometry {
        self.fat_image.geometry()
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    fn sync(&mut self) -> Result<()> {
        self.sync_to_host()
    }
}

/// Scan a directory and return all files and subdirectories.
/// Uses tilde notation (e.g. LONGFI~1.COM, LONGFI~2.COM) to resolve 8.3 name collisions.
fn scan_directory(path: &Path) -> Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    let base_path = path;

    // Track used DOS names per parent DOS path (empty string = root).
    let mut used_names: HashMap<String, HashSet<String>> = HashMap::new();
    // Map host relative path → already-assigned DOS path, so nested entries can
    // look up their parent's name rather than re-deriving (and potentially colliding).
    let mut path_to_dos: HashMap<String, String> = HashMap::new();

    for entry in WalkDir::new(path).follow_links(false).sort_by_file_name() {
        let entry = entry.context("Failed to read directory entry")?;
        let host_path = entry.path();

        let rel_path = host_path
            .strip_prefix(base_path)
            .context("Failed to strip base path")?;

        if rel_path.as_os_str().is_empty() {
            continue; // Skip root
        }

        let metadata = entry.metadata().context("Failed to get file metadata")?;

        let components: Vec<_> = rel_path.components().collect();

        // Look up the already-assigned DOS path for the parent directory.
        let parent_dos_path = if components.len() <= 1 {
            String::new()
        } else {
            let parent_rel: PathBuf = components[..components.len() - 1].iter().collect();
            path_to_dos
                .get(&parent_rel.to_string_lossy().to_string())
                .cloned()
                .unwrap_or_default()
        };

        // Assign a unique DOS name for only the last component (the current entry).
        let name = components.last().unwrap().as_os_str().to_string_lossy();
        let dir_used = used_names.entry(parent_dos_path.clone()).or_default();
        let dos_name = to_dos_name_unique(&name, dir_used);
        dir_used.insert(dos_name.clone());

        let dos_path = if parent_dos_path.is_empty() {
            dos_name
        } else {
            format!("{}/{}", parent_dos_path, dos_name)
        };

        // Record the mapping so children can look up their parent's DOS path.
        path_to_dos.insert(rel_path.to_string_lossy().to_string(), dos_path.clone());

        entries.push(FileEntry {
            dos_path,
            host_path: host_path.to_path_buf(),
            size: metadata.len(),
            is_dir: metadata.is_dir(),
        });
    }

    Ok(entries)
}

/// Convert a single filename to DOS 8.3 format, using tilde notation if the plain
/// truncated name is already taken in `used_names`.
///
/// Examples:
///   "test_video_mode1.com" (first)  → "TEST_VID.COM"
///   "test_video_mode2.com" (second) → "TESTV~1.COM"
///   "test_video_mode3.com" (third)  → "TESTV~2.COM"
fn to_dos_name_unique(name: &str, used_names: &HashSet<String>) -> String {
    let (base, ext) = if let Some(dot_pos) = name.rfind('.') {
        (&name[..dot_pos], Some(&name[dot_pos + 1..]))
    } else {
        (name, None)
    };

    let base_up = base.to_uppercase();
    let ext_up = ext.map(|e| e[..e.len().min(3)].to_uppercase());

    let make_name = |b: &str| -> String {
        match &ext_up {
            Some(e) => format!("{}.{}", b, e),
            None => b.to_string(),
        }
    };

    // Try plain truncated name first.
    let plain = make_name(&base_up[..base_up.len().min(8)]);
    if !used_names.contains(&plain) {
        return plain;
    }

    // Generate tilde alternatives: truncate base to 6 chars + ~N.
    let tilde_base = &base_up[..base_up.len().min(6)];
    for n in 1u32.. {
        let suffix = format!("~{}", n);
        let candidate = make_name(&format!("{}{}", tilde_base, suffix));
        if !used_names.contains(&candidate) {
            return candidate;
        }
    }

    unreachable!("could not generate unique DOS name")
}

/// Calculate appropriate disk geometry for the given data size.
fn calculate_geometry(data_bytes: u64) -> Result<DiskGeometry> {
    // Add 20% overhead for FAT structures
    let total_bytes = (data_bytes as f64 * 1.2) as u64;

    // Round up to power of 2, minimum 16MB
    let min_sectors = 32768; // 16 MB
    let total_sectors =
        ((total_bytes / SECTOR_SIZE as u64).max(min_sectors)).next_power_of_two() as usize;

    // Create hard drive geometry
    Ok(DiskGeometry::hard_drive(total_sectors))
}

/// Populate a FAT image with files from the host directory.
fn populate_fat_image(
    disk: &mut BackedDisk<MemoryDiskBackend>,
    files: Vec<FileEntry>,
) -> Result<usize> {
    let mut file_count = 0;

    // For hard drives, FAT partition starts at sector 63 (after MBR)
    const PARTITION_START: usize = 63;
    const SECTOR_SIZE: usize = 512;
    let partition_offset = PARTITION_START * SECTOR_SIZE;

    // Get full disk data
    let backend = disk.backend().clone();
    let disk_data = backend.get_data();

    // Create a cursor view of just the partition
    let partition_data = &disk_data[partition_offset..];
    let mut cursor: std::io::Cursor<Vec<u8>> = std::io::Cursor::new(partition_data.to_vec());

    let fs = fatfs::FileSystem::new(&mut cursor, fatfs::FsOptions::new())
        .context("Failed to mount FAT partition")?;

    let root = fs.root_dir();

    // Create directories first (so parent directories exist before files)
    let mut dirs: Vec<_> = files.iter().filter(|e| e.is_dir).collect();
    dirs.sort_by_key(|e| e.dos_path.matches('/').count()); // Create shallow dirs first

    for entry in dirs {
        match root.create_dir(&entry.dos_path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(anyhow!(
                    "Failed to create directory {}: {}",
                    entry.dos_path,
                    e
                ));
            }
        }
    }

    // Create files
    for entry in files.iter().filter(|e| !e.is_dir) {
        let host_data = fs::read(&entry.host_path)
            .with_context(|| format!("Failed to read {}", entry.host_path.display()))?;

        let mut file = root
            .create_file(&entry.dos_path)
            .with_context(|| format!("Failed to create FAT file: {}", entry.dos_path))?;

        file.write_all(&host_data)
            .with_context(|| format!("Failed to write to FAT file: {}", entry.dos_path))?;

        file_count += 1;
    }

    // Write changes back to disk
    drop(root);
    drop(fs);

    // Copy the updated partition data back into the full disk (preserving MBR)
    let updated_partition = cursor.into_inner();
    let mut full_disk_data = disk_data.clone();
    let end_offset = partition_offset + updated_partition.len();
    full_disk_data[partition_offset..end_offset].copy_from_slice(&updated_partition);

    let new_backend = MemoryDiskBackend::new(full_disk_data);
    *disk = BackedDisk::new(new_backend)?;

    Ok(file_count)
}

/// Sync a FAT directory recursively to the host filesystem.
fn sync_directory<IO>(fat_dir: &fatfs::Dir<IO>, host_base: &Path, rel_path: &str) -> Result<()>
where
    IO: std::io::Read + std::io::Write + std::io::Seek,
{
    let host_dir = if rel_path.is_empty() {
        host_base.to_path_buf()
    } else {
        host_base.join(rel_path.replace('/', std::path::MAIN_SEPARATOR_STR))
    };

    // Ensure directory exists
    fs::create_dir_all(&host_dir)
        .with_context(|| format!("Failed to create directory: {}", host_dir.display()))?;

    for entry in fat_dir.iter() {
        let entry = entry.context("Failed to read FAT directory entry")?;
        let name = entry.file_name();

        // Skip volume labels and special entries
        if name == "." || name == ".." {
            continue;
        }

        let rel_entry_path = if rel_path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", rel_path, name)
        };

        if entry.is_dir() {
            // Recurse into subdirectory
            let subdir = entry.to_dir();
            sync_directory(&subdir, host_base, &rel_entry_path)?;
        } else {
            // Write file to host
            let host_file_path =
                host_base.join(rel_entry_path.replace('/', std::path::MAIN_SEPARATOR_STR));
            let mut fat_file = entry.to_file();
            let mut contents = Vec::new();
            fat_file
                .read_to_end(&mut contents)
                .context("Failed to read FAT file")?;

            fs::write(&host_file_path, &contents)
                .with_context(|| format!("Failed to write file: {}", host_file_path.display()))?;
        }
    }

    Ok(())
}
