//! Host directory mounting as DOS drives
//!
//! Allows mounting a host filesystem directory as a FAT-formatted DOS drive.
//! Files are scanned from the host, a FAT filesystem is generated in memory,
//! and changes are synced back to the host on close/shutdown.

use anyhow::{Context, Result, anyhow};
use emu86_core::{
    BackedDisk, DiskController, DiskGeometry, MemoryDiskBackend, SECTOR_SIZE,
};
use std::collections::HashSet;
use std::fs;
use std::io::{Cursor, Read as _, Write as _};
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

        // 3. Create and format blank FAT image (without MBR - direct FAT formatting)
        let disk_data = vec![0u8; geometry.total_size];
        let mut cursor = Cursor::new(disk_data);
        let format_opts = fatfs::FormatVolumeOptions::new()
            .volume_label(*b"HOST_DIR   ");
        fatfs::format_volume(&mut cursor, format_opts)
            .map_err(|e| anyhow!("Failed to format volume: {}", e))?;

        let disk_data = cursor.into_inner();
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

        // Extract all files from the FAT image using fatfs
        let backend = self.fat_image.backend().clone();
        let mut adapter = MemoryDiskAdapter::new(backend);

        let fs = fatfs::FileSystem::new(&mut adapter, fatfs::FsOptions::new())
            .context("Failed to mount FAT filesystem for sync")?;

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
fn scan_directory(path: &Path) -> Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    let base_path = path;

    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry.context("Failed to read directory entry")?;
        let host_path = entry.path();

        // Get relative path from base
        let rel_path = host_path
            .strip_prefix(base_path)
            .context("Failed to strip base path")?;

        if rel_path.as_os_str().is_empty() {
            continue; // Skip root
        }

        // Convert to DOS path format
        let dos_path = to_dos_path(rel_path)?;

        let metadata = entry.metadata().context("Failed to get file metadata")?;

        entries.push(FileEntry {
            dos_path,
            host_path: host_path.to_path_buf(),
            size: metadata.len(),
            is_dir: metadata.is_dir(),
        });
    }

    Ok(entries)
}

/// Convert a Unix path to DOS format with 8.3 filenames.
fn to_dos_path(path: &Path) -> Result<String> {
    let mut result = String::new();

    for component in path.components() {
        if !result.is_empty() {
            result.push('/');
        }

        let name = component.as_os_str().to_string_lossy();

        // Convert to 8.3: "long_filename.txt" -> "LONG_FIL.TXT"
        let dos_name = to_dos_name(&name);
        result.push_str(&dos_name);
    }

    Ok(result)
}

/// Convert a single filename to DOS 8.3 format.
fn to_dos_name(name: &str) -> String {
    let (base, ext) = if let Some(dot_pos) = name.rfind('.') {
        (&name[..dot_pos], Some(&name[dot_pos + 1..]))
    } else {
        (name, None)
    };

    // Truncate base to 8 chars, extension to 3 chars, convert to uppercase
    let base = &base[..base.len().min(8)].to_uppercase();

    if let Some(ext) = ext {
        let ext = &ext[..ext.len().min(3)].to_uppercase();
        format!("{}.{}", base, ext)
    } else {
        base.to_string()
    }
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

    // Create a fatfs adapter
    let backend = disk.backend().clone();
    let mut adapter = MemoryDiskAdapter::new(backend);

    let fs = fatfs::FileSystem::new(&mut adapter, fatfs::FsOptions::new())
        .context("Failed to mount FAT filesystem")?;

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

    // Copy the updated data back to the disk
    let updated_data = adapter.into_inner();
    *disk = BackedDisk::new(updated_data)?;

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

/// Adapter for MemoryDiskBackend to work with fatfs.
/// Similar to DiskAdapter in core, but for MemoryDiskBackend specifically.
struct MemoryDiskAdapter {
    backend: MemoryDiskBackend,
    position: u64,
}

impl MemoryDiskAdapter {
    fn new(backend: MemoryDiskBackend) -> Self {
        Self {
            backend,
            position: 0,
        }
    }

    fn into_inner(self) -> MemoryDiskBackend {
        self.backend
    }
}

impl std::io::Read for MemoryDiskAdapter {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use emu86_core::DiskBackend;
        let bytes_read = self
            .backend
            .read_at(self.position, buf)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
}

impl std::io::Write for MemoryDiskAdapter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use emu86_core::DiskBackend;
        let bytes_written = self
            .backend
            .write_at(self.position, buf)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        self.position += bytes_written as u64;
        Ok(bytes_written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        use emu86_core::DiskBackend;
        self.backend
            .flush()
            .map_err(|e| std::io::Error::other(e.to_string()))
    }
}

impl std::io::Seek for MemoryDiskAdapter {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let size = self.backend.size();
        let new_pos = match pos {
            std::io::SeekFrom::Start(offset) => offset as i64,
            std::io::SeekFrom::End(offset) => size as i64 + offset,
            std::io::SeekFrom::Current(offset) => self.position as i64 + offset,
        };

        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Seek before start of disk",
            ));
        }

        self.position = new_pos as u64;
        Ok(self.position)
    }
}
