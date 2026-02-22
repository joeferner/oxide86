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
    read_only: bool,
    /// Maps DOS path (e.g. ".GIT/HOOKS/APPLYPAT.SAM") → original host path.
    /// Used during sync to write files back to their original names/casing.
    name_map: HashMap<String, PathBuf>,
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

        // 3. Create disk image with MBR at sector 0 and FAT partition starting at sector 63.
        // The MBR is required so DOS can probe the drive via INT 13h and recognise it as a
        // valid hard drive with a partition. The FAT partition itself is accessed by wrapping
        // this disk in a PartitionedDisk (see load_mounted_directories in setup.rs), which
        // offsets all sector reads/writes by PARTITION_START so that fatfs sees the FAT
        // boot sector at its logical sector 0.
        let disk_data = create_formatted_disk(geometry, Some("HOST_DIR"))?;
        let backend = MemoryDiskBackend::new(disk_data);
        let mut disk = BackedDisk::new(backend)?;

        // 4. Populate FAT image with files from host
        let file_count = populate_fat_image(&mut disk, &files)?;
        log::info!("Populated FAT image with {} files", file_count);

        // Build name_map: DOS path → original host path (for both files and dirs)
        let name_map: HashMap<String, PathBuf> = files
            .into_iter()
            .map(|e| (e.dos_path, e.host_path))
            .collect();

        Ok(Self {
            host_path,
            fat_image: disk,
            read_only,
            name_map,
        })
    }

    /// Number of sectors in the FAT partition (total sectors minus MBR reserved sectors).
    pub fn partition_sectors(&self) -> usize {
        const PARTITION_START: usize = 63;
        self.fat_image.geometry().total_sectors() - PARTITION_START
    }

    /// Create a raw (full-disk) view of the underlying memory for INT 13h access.
    /// This shares the same memory as the FAT image, so writes through the partition
    /// view (PartitionedDisk) are immediately visible here.
    pub fn create_raw_disk(&self) -> BackedDisk<MemoryDiskBackend> {
        let backend = self.fat_image.backend().clone();
        BackedDisk::new(backend).expect("HostDirectoryDisk: raw disk view creation failed")
    }

    /// Sync changes from the FAT image back to the host directory.
    /// This extracts all files from the FAT image and writes them to the host.
    pub fn sync_to_host(&mut self) -> Result<()> {
        if self.read_only {
            return Ok(());
        }

        log::info!("Syncing to host: {}", self.host_path.display());

        // FAT partition starts at sector 63 (after MBR).
        // When booted DOS uses INT 13h directly, writes go to the shared raw_adapter memory
        // (same Rc as fat_image), so we always read the current FAT state unconditionally.
        const PARTITION_START: usize = 63;
        let partition_offset = PARTITION_START * SECTOR_SIZE;

        let backend = self.fat_image.backend().clone();
        let disk_data = backend.get_data();
        let partition_data = disk_data[partition_offset..].to_vec();
        let mut cursor: std::io::Cursor<Vec<u8>> = std::io::Cursor::new(partition_data);

        let fs = fatfs::FileSystem::new(&mut cursor, fatfs::FsOptions::new())
            .context("Failed to mount FAT partition for sync")?;

        let (files_written, dirs_created) =
            sync_directory(&fs.root_dir(), &self.host_path, "", &self.name_map)?;

        log::info!(
            "Sync complete: {} file(s) written, {} director(ies) created",
            files_written,
            dirs_created
        );

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
        self.fat_image.write_sector_lba(lba, data)
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
    files: &[FileEntry],
) -> Result<usize> {
    let mut file_count = 0;

    // FAT partition starts at sector 63 (after MBR).
    const PARTITION_START: usize = 63;
    const SECTOR_SIZE_LOCAL: usize = 512;
    let partition_offset = PARTITION_START * SECTOR_SIZE_LOCAL;

    let backend = disk.backend().clone();
    let disk_data = backend.get_data();
    let partition_data = disk_data[partition_offset..].to_vec();
    let mut cursor: std::io::Cursor<Vec<u8>> = std::io::Cursor::new(partition_data);

    let fs = fatfs::FileSystem::new(&mut cursor, fatfs::FsOptions::new())
        .context("Failed to mount FAT partition for population")?;

    let root = fs.root_dir();

    // Create directories first (so parent directories exist before files)
    let mut dirs: Vec<_> = files.iter().filter(|e| e.is_dir).collect();
    dirs.sort_by_key(|e| e.dos_path.matches('/').count()); // Create shallow dirs first

    for entry in &dirs {
        log::debug!("  Adding dir:  {}", entry.dos_path);
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
        log::debug!("  Adding file: {} ({} bytes)", entry.dos_path, entry.size);
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

    // Copy updated partition data back into the full disk image (preserving MBR).
    let updated_partition = cursor.into_inner();
    let mut full_disk_data = disk_data;
    full_disk_data[partition_offset..partition_offset + updated_partition.len()]
        .copy_from_slice(&updated_partition);
    let new_backend = MemoryDiskBackend::new(full_disk_data);
    *disk = BackedDisk::new(new_backend)?;

    Ok(file_count)
}

/// Sync a FAT directory recursively to the host filesystem.
///
/// `host_dir` is the actual host path for the current directory.
/// `dos_rel_path` is the DOS-relative path used as key into `name_map`.
/// `name_map` maps DOS paths → original host paths for files/dirs that existed at mount time.
///
/// Returns (files_written, dirs_created).
fn sync_directory<IO>(
    fat_dir: &fatfs::Dir<IO>,
    host_dir: &Path,
    dos_rel_path: &str,
    name_map: &HashMap<String, PathBuf>,
) -> Result<(usize, usize)>
where
    IO: std::io::Read + std::io::Write + std::io::Seek,
{
    let mut files_written = 0usize;
    let mut dirs_created = 0usize;

    // Ensure the current directory exists on the host (root always exists)
    if !host_dir.exists() {
        log::info!("  Creating dir: {}", host_dir.display());
        fs::create_dir_all(host_dir)
            .with_context(|| format!("Failed to create directory: {}", host_dir.display()))?;
        dirs_created += 1;
    }

    for entry in fat_dir.iter() {
        let entry = entry.context("Failed to read FAT directory entry")?;
        let name = entry.file_name();

        if name == "." || name == ".." {
            continue;
        }

        // Build the DOS-relative path for this entry (used as name_map key)
        let child_dos_path = if dos_rel_path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", dos_rel_path, name)
        };

        // Resolve the host path: use the original path from name_map if available
        // (preserves original casing and long filenames), otherwise lowercase the DOS name.
        let child_host_path = if let Some(original) = name_map.get(&child_dos_path) {
            original.clone()
        } else {
            host_dir.join(name.to_lowercase())
        };

        if entry.is_dir() {
            let subdir = entry.to_dir();
            let (f, d) = sync_directory(&subdir, &child_host_path, &child_dos_path, name_map)?;
            files_written += f;
            dirs_created += d;
        } else {
            let mut fat_file = entry.to_file();
            let mut contents = Vec::new();
            fat_file
                .read_to_end(&mut contents)
                .context("Failed to read FAT file")?;

            // Only write if content has changed (or file is new)
            let changed = match fs::read(&child_host_path) {
                Ok(host_contents) => host_contents != contents,
                Err(_) => true,
            };

            if changed {
                // Ensure parent directory exists for new files in new dirs
                if let Some(parent) = child_host_path.parent()
                    && !parent.exists()
                {
                    fs::create_dir_all(parent).with_context(|| {
                        format!("Failed to create parent dir: {}", parent.display())
                    })?;
                }
                log::info!(
                    "  Writing file: {} ({} bytes)",
                    child_host_path.display(),
                    contents.len()
                );
                fs::write(&child_host_path, &contents).with_context(|| {
                    format!("Failed to write file: {}", child_host_path.display())
                })?;
                files_written += 1;
            } else {
                log::debug!("  Unchanged: {}", child_host_path.display());
            }
        }
    }

    Ok((files_written, dirs_created))
}
