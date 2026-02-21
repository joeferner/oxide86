//! Multi-drive management for the 8086 emulator
//!
//! Supports:
//! - 2 floppy drive slots (A: = 0x00, B: = 0x01) - can be empty or contain disk
//! - Multiple hard drives (C: = 0x80, D: = 0x81, etc.) - always present once added
//! - Per-drive current directory tracking
//! - Disk change detection flags for floppies

use crate::DriveNumber;
use crate::cdrom::{CdRomImage, IsoEntry};
use crate::cpu::bios::disk_error::DiskError;
use crate::cpu::bios::dos_error::DosError;
use crate::cpu::bios::{DriveParams, FileAccess, FindData, SeekMethod};
use crate::disk::{DiskController, SECTOR_SIZE};
use std::collections::HashMap;
use std::io::{self, Error, ErrorKind, Read, Seek, SeekFrom, Write};

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// Adapter to make DiskController work with fatfs crate's ReadWriteSeek trait
pub struct DiskAdapter {
    disk: Box<dyn DiskController>,
    position: u64,
}

impl DiskAdapter {
    pub fn new(disk: Box<dyn DiskController>) -> Self {
        Self { disk, position: 0 }
    }

    /// Get the total size of the disk in bytes
    fn size(&self) -> u64 {
        self.disk.size() as u64
    }

    /// Get mutable access to the underlying disk (for pass-through operations)
    pub fn disk_mut(&mut self) -> &mut Box<dyn DiskController> {
        &mut self.disk
    }

    /// Get immutable access to the underlying disk
    pub fn disk(&self) -> &dyn DiskController {
        &self.disk
    }

    /// Reset adapter position to 0 (required before fatfs FileSystem::new())
    pub fn reset_position(&mut self) {
        self.position = 0;
    }

    /// Consume adapter and return the underlying disk
    pub fn into_disk(self) -> Box<dyn DiskController> {
        self.disk
    }
}

impl Read for DiskAdapter {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.size() {
            return Ok(0); // EOF
        }

        let bytes_to_read = buf.len().min((self.size() - self.position) as usize);
        let start_sector = (self.position / SECTOR_SIZE as u64) as usize;
        let offset_in_sector = (self.position % SECTOR_SIZE as u64) as usize;

        let mut bytes_read = 0;
        let mut current_sector = start_sector;
        let mut buf_offset = 0;

        while bytes_read < bytes_to_read {
            let sector_data = self
                .disk
                .read_sector_lba(current_sector)
                .map_err(|e| Error::other(e.to_string()))?;

            let sector_offset = if bytes_read == 0 { offset_in_sector } else { 0 };
            let bytes_in_sector = (SECTOR_SIZE - sector_offset).min(bytes_to_read - bytes_read);

            buf[buf_offset..buf_offset + bytes_in_sector]
                .copy_from_slice(&sector_data[sector_offset..sector_offset + bytes_in_sector]);

            bytes_read += bytes_in_sector;
            buf_offset += bytes_in_sector;
            current_sector += 1;
        }

        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
}

impl Write for DiskAdapter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.disk.is_read_only() {
            return Err(Error::new(ErrorKind::PermissionDenied, "Disk is read-only"));
        }

        if self.position >= self.size() {
            return Err(Error::new(ErrorKind::WriteZero, "Write beyond disk end"));
        }

        let bytes_to_write = buf.len().min((self.size() - self.position) as usize);
        let start_sector = (self.position / SECTOR_SIZE as u64) as usize;
        let offset_in_sector = (self.position % SECTOR_SIZE as u64) as usize;

        let mut bytes_written = 0;
        let mut current_sector = start_sector;
        let mut buf_offset = 0;

        while bytes_written < bytes_to_write {
            let sector_offset = if bytes_written == 0 {
                offset_in_sector
            } else {
                0
            };
            let bytes_in_sector = (SECTOR_SIZE - sector_offset).min(bytes_to_write - bytes_written);

            // If not writing full sector, need read-modify-write
            let mut sector_data = if sector_offset != 0 || bytes_in_sector < SECTOR_SIZE {
                self.disk
                    .read_sector_lba(current_sector)
                    .map_err(|e| Error::other(e.to_string()))?
            } else {
                [0u8; SECTOR_SIZE]
            };

            sector_data[sector_offset..sector_offset + bytes_in_sector]
                .copy_from_slice(&buf[buf_offset..buf_offset + bytes_in_sector]);

            self.disk
                .write_sector_lba(current_sector, &sector_data)
                .map_err(|e| Error::other(e.to_string()))?;

            bytes_written += bytes_in_sector;
            buf_offset += bytes_in_sector;
            current_sector += 1;
        }

        self.position += bytes_written as u64;
        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(()) // Disk writes are synchronous
    }
}

impl Seek for DiskAdapter {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_position = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => self.position as i64 + offset,
            SeekFrom::End(offset) => self.size() as i64 + offset,
        };

        if new_position < 0 {
            return Err(Error::new(ErrorKind::InvalidInput, "Seek before start"));
        }

        self.position = new_position as u64;
        Ok(self.position)
    }
}

/// State for a single drive slot
pub struct DriveState {
    /// The disk adapter for filesystem operations (None for empty floppy slot)
    adapter: Option<DiskAdapter>,
    /// Raw disk adapter for INT 13h BIOS operations (Some for partitioned hard drives)
    /// This allows reading the MBR which is not part of the partition
    raw_adapter: Option<DiskAdapter>,
    /// Current working directory for this drive (Unix-style path with forward slashes)
    current_dir: String,
    /// Disk change flag (true if disk was changed since last check)
    disk_changed: bool,
    /// Whether this is a removable drive (floppy)
    removable: bool,
}

impl DriveState {
    /// Create a new drive state with a disk
    pub fn new_with_disk(disk: Box<dyn DiskController>, removable: bool) -> Self {
        Self {
            adapter: Some(DiskAdapter::new(disk)),
            raw_adapter: None,
            current_dir: String::from("/"),
            disk_changed: false,
            removable,
        }
    }

    /// Create a new drive state with both partitioned and raw disk adapters
    /// Used for partitioned hard drives where INT 13h needs raw MBR access
    pub fn new_with_partition(
        partition: Box<dyn DiskController>,
        raw_disk: Box<dyn DiskController>,
        removable: bool,
    ) -> Self {
        Self {
            adapter: Some(DiskAdapter::new(partition)),
            raw_adapter: Some(DiskAdapter::new(raw_disk)),
            current_dir: String::from("/"),
            disk_changed: false,
            removable,
        }
    }

    /// Create an empty drive slot (for removable drives)
    pub fn empty() -> Self {
        Self {
            adapter: None,
            raw_adapter: None,
            current_dir: String::from("/"),
            disk_changed: false,
            removable: true,
        }
    }

    /// Check if drive has a disk inserted
    pub fn has_disk(&self) -> bool {
        self.adapter.is_some()
    }
}

/// Metadata for an open file handle
struct FileHandle {
    /// Which drive this file is on
    drive: DriveNumber,
    /// Path on that drive
    path: String,
    /// Current position in file
    position: u64,
    /// Access mode (read/write/both)
    access_mode: FileAccess,
    /// For CD-ROM files: entire file content buffered at open time
    cdrom_data: Option<Vec<u8>>,
    /// Total size of the CD-ROM file (for SeekMethod::FromEnd)
    cdrom_size: u64,
}

/// Search state for directory iteration
struct SearchState {
    /// Which drive this search is on
    drive: DriveNumber,
    /// Matching entries
    entries: Vec<FindData>,
    /// Current index in entries
    index: usize,
}

/// Manages multiple drives with unified handle allocation
///
/// Drive numbering:
/// - 0x00 = Floppy A:
/// - 0x01 = Floppy B:
/// - 0x80 = Hard drive C:
/// - 0x81 = Hard drive D:
/// - 0xE0 = CD-ROM slot 0
/// - 0xE1 = CD-ROM slot 1
/// - etc.
pub struct DriveManager {
    /// Floppy drive slots (A: = 0, B: = 1)
    floppy_drives: [DriveState; 2],

    /// Hard drives (C: = 0x80, D: = 0x81, etc.)
    hard_drives: Vec<DriveState>,

    /// CD-ROM drives (0xE0 = slot 0, 0xE1 = slot 1, etc.)
    cdrom_drives: [Option<CdRomImage>; 4],

    /// Current default drive (0 = A:, 1 = B:, 0x80 = C:, etc.)
    current_drive: DriveNumber,

    /// Open file handles: maps handle -> file metadata
    open_files: HashMap<u16, FileHandle>,

    /// Search states for find first/next
    searches: HashMap<usize, SearchState>,

    /// Next file handle to allocate (global across all drives)
    next_handle: u16,

    /// Next search ID
    next_search_id: usize,
}

impl Default for DriveManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DriveManager {
    /// Create a new drive manager with empty floppy slots and no hard drives
    pub fn new() -> Self {
        Self {
            floppy_drives: [DriveState::empty(), DriveState::empty()],
            hard_drives: Vec::new(),
            cdrom_drives: [None, None, None, None],
            current_drive: DriveNumber::from_standard(0),
            open_files: HashMap::new(),
            searches: HashMap::new(),
            next_handle: 3, // 0-2 reserved for stdin/stdout/stderr
            next_search_id: 0,
        }
    }

    /// Set the next handle number (for platform synchronization with device handles)
    pub fn set_next_handle(&mut self, handle: u16) {
        self.next_handle = handle;
    }

    /// Get the next handle number (for platform synchronization)
    pub fn get_next_handle(&self) -> u16 {
        self.next_handle
    }

    /// Close all open files and clear searches (used during reset)
    pub fn close_all_files(&mut self) {
        self.open_files.clear();
        self.searches.clear();
        self.next_handle = 3; // Reset to initial value
        self.next_search_id = 0;
    }

    // === Floppy Management ===

    /// Insert a floppy disk into a slot (0 = A:, 1 = B:)
    pub fn insert_floppy(
        &mut self,
        slot: DriveNumber,
        disk: Box<dyn DiskController>,
    ) -> Result<(), String> {
        if !slot.is_floppy() {
            return Err(format!("Invalid floppy slot: {} (must be 0 or 1)", slot));
        }

        // Close any open files on this drive first
        self.close_files_on_drive(slot);

        self.floppy_drives[slot.to_floppy_index()] = DriveState::new_with_disk(disk, true);
        self.floppy_drives[slot.to_floppy_index()].disk_changed = true; // Mark as changed

        Ok(())
    }

    /// Eject a floppy disk from a slot, returning the disk if present
    pub fn eject_floppy(
        &mut self,
        slot: DriveNumber,
    ) -> Result<Option<Box<dyn DiskController>>, String> {
        if !slot.is_floppy() {
            return Err(format!("Invalid floppy slot: {} (must be 0 or 1)", slot));
        }

        // Close any open files on this drive
        self.close_files_on_drive(slot);

        let old_state = std::mem::replace(
            &mut self.floppy_drives[slot.to_floppy_index()],
            DriveState::empty(),
        );

        // Return the disk if there was one
        Ok(old_state.adapter.map(|a| a.into_disk()))
    }

    // === Hard Drive Management ===

    /// Add a hard drive (returns the assigned drive number: 0x80, 0x81, etc.)
    pub fn add_hard_drive(&mut self, disk: Box<dyn DiskController>) -> DriveNumber {
        let drive_number = DriveNumber::from_hard_drive_index(self.hard_drives.len());
        self.hard_drives
            .push(DriveState::new_with_disk(disk, false));
        drive_number
    }

    /// Add a partitioned hard drive with both partition and raw disk views
    /// partition: Disk with partition offset (for DOS filesystem operations)
    /// raw_disk: Raw disk without offset (for INT 13h MBR access)
    pub fn add_hard_drive_with_partition(
        &mut self,
        partition: Box<dyn DiskController>,
        raw_disk: Box<dyn DiskController>,
    ) -> DriveNumber {
        let drive_number = DriveNumber::from_hard_drive_index(self.hard_drives.len());
        self.hard_drives
            .push(DriveState::new_with_partition(partition, raw_disk, false));
        drive_number
    }

    // === CD-ROM Management ===

    /// Insert a CD-ROM ISO image into a slot (0-3).
    /// Returns the assigned drive number (0xE0-0xE3).
    pub fn insert_cdrom(&mut self, slot: u8, image: CdRomImage) -> DriveNumber {
        let slot = slot.min(3) as usize;
        let drive = DriveNumber::cdrom(slot as u8);
        self.close_files_on_drive(drive);
        self.cdrom_drives[slot] = Some(image);
        drive
    }

    /// Eject a CD-ROM from a slot, returning the image if present.
    pub fn eject_cdrom(&mut self, slot: u8) -> Option<CdRomImage> {
        let slot = slot.min(3) as usize;
        let drive = DriveNumber::cdrom(slot as u8);
        self.close_files_on_drive(drive);
        self.cdrom_drives[slot].take()
    }

    /// Check if a CD-ROM slot has a disc inserted.
    pub fn has_cdrom(&self, slot: u8) -> bool {
        let slot = slot as usize;
        slot < 4 && self.cdrom_drives[slot].is_some()
    }

    /// Return the number of CD-ROM drives with a disc inserted.
    pub fn cdrom_count(&self) -> u8 {
        self.cdrom_drives.iter().filter(|d| d.is_some()).count() as u8
    }

    /// Read a 512-byte sub-sector from a CD-ROM drive.
    ///
    /// CD-ROM sectors are 2048 bytes; this maps 512-byte LBA indices by dividing
    /// by 4 (cd_lba = lba_512 / 4, offset_in_cd_sector = (lba_512 % 4) * 512).
    pub fn cdrom_read_sector_as_512(
        &self,
        drive: DriveNumber,
        lba_512: usize,
    ) -> Result<[u8; SECTOR_SIZE], DiskError> {
        if !drive.is_cdrom() {
            return Err(DiskError::DriveNotReady);
        }
        let slot = drive.cdrom_slot() as usize;
        let image = self.cdrom_drives[slot]
            .as_ref()
            .ok_or(DiskError::DriveNotReady)?;

        let cd_lba = lba_512 / 4;
        let offset_in_cd = (lba_512 % 4) * SECTOR_SIZE;

        let cd_sector = image
            .read_sector(cd_lba as u32)
            .map_err(|_| DiskError::SectorNotFound)?;

        let mut out = [0u8; SECTOR_SIZE];
        out.copy_from_slice(&cd_sector[offset_in_cd..offset_in_cd + SECTOR_SIZE]);
        Ok(out)
    }

    /// Convert an ISO entry to FindData for directory searches.
    fn iso_entry_to_find_data(entry: &IsoEntry) -> FindData {
        let (dos_date, dos_time) = CdRomImage::dos_date_from_iso(&entry.recording_date);
        FindData {
            attributes: if entry.is_dir { 0x10 } else { 0x01 }, // DIR or ReadOnly
            time: dos_time,
            date: dos_date,
            size: entry.data_length,
            filename: entry.name.clone(),
        }
    }

    /// Sync a drive's changes to backing storage (e.g., for host directory mounts)
    pub fn sync_drive(&mut self, drive: DriveNumber) -> Result<(), String> {
        let drive_state = self
            .get_drive_mut(drive)
            .ok_or_else(|| format!("Drive {} not found", drive))?;

        if let Some(adapter) = &mut drive_state.adapter {
            adapter
                .disk_mut()
                .sync()
                .map_err(|e| format!("Failed to sync drive {}: {}", drive, e))?;
        }

        Ok(())
    }

    // === Drive Access ===

    /// Get mutable reference to a drive state
    fn get_drive_mut(&mut self, drive: DriveNumber) -> Option<&mut DriveState> {
        if drive.is_floppy() {
            // Floppy drive
            if drive.to_floppy_index() < 2 {
                Some(&mut self.floppy_drives[drive.to_floppy_index()])
            } else {
                None
            }
        } else {
            // Hard drive
            self.hard_drives.get_mut(drive.to_hard_drive_index())
        }
    }

    /// Get immutable reference to a drive state
    fn get_drive(&self, drive: DriveNumber) -> Option<&DriveState> {
        if drive.is_floppy() {
            if drive.to_floppy_index() < 2 {
                Some(&self.floppy_drives[drive.to_floppy_index()])
            } else {
                None
            }
        } else {
            self.hard_drives.get(drive.to_hard_drive_index())
        }
    }

    /// Close all open files on a specific drive
    fn close_files_on_drive(&mut self, drive: DriveNumber) {
        let handles_to_close: Vec<u16> = self
            .open_files
            .iter()
            .filter(|(_, fh)| fh.drive == drive)
            .map(|(&h, _)| h)
            .collect();

        for handle in handles_to_close {
            self.open_files.remove(&handle);
        }

        // Also clear any searches on this drive
        let searches_to_clear: Vec<usize> = self
            .searches
            .iter()
            .filter(|(_, s)| s.drive == drive)
            .map(|(&id, _)| id)
            .collect();

        for id in searches_to_clear {
            self.searches.remove(&id);
        }
    }

    // === Current Drive/Directory Management ===

    /// Get the current default drive
    pub fn get_current_drive(&self) -> DriveNumber {
        self.current_drive
    }

    /// Set the current default drive
    /// drive: DOS drive number (0=A, 1=B, 2=C, etc.)
    /// Returns the total number of logical drives on success
    pub fn set_current_drive(&mut self, drive: DriveNumber) -> Result<u8, DosError> {
        // Verify drive exists
        if self.get_drive(drive).is_some() {
            self.current_drive = drive;
            // Return number of logical drives (floppies + hard drives)
            let floppy_count = 2u8; // Always report 2 floppy slots
            let hd_count = self.hard_drives.len() as u8;
            Ok(floppy_count + hd_count)
        } else {
            Err(DosError::InvalidDrive)
        }
    }

    /// Get current directory for a drive
    /// drive: None = use current default drive
    pub fn get_current_dir(&self, drive: DriveNumber) -> Result<String, DosError> {
        self.get_drive(drive)
            .map(|d| {
                // Convert Unix path back to DOS path
                d.current_dir.replace('/', "\\")
            })
            .ok_or(DosError::InvalidDrive)
    }

    /// Get total number of drives
    pub fn get_drive_count(&self) -> u8 {
        2 + self.hard_drives.len() as u8
    }

    // === Path Parsing ===

    /// Parse a DOS path into (drive_number, path)
    /// Examples:
    ///   "C:\FOO\BAR" -> (0x80, "/FOO/BAR")
    ///   "A:FILE.TXT" -> (0x00, "FILE.TXT")
    ///   "\TEMP"      -> (current_drive, "/TEMP")
    ///   "FILE.TXT"   -> (current_drive, "FILE.TXT")
    fn parse_path(&self, path: &str) -> (DriveNumber, String) {
        let path = path.replace('\\', "/");

        if path.len() >= 2 && path.chars().nth(1) == Some(':') {
            // Has drive letter
            let drive_letter = path.chars().next().unwrap().to_ascii_uppercase();
            let drive = if let Some(drive) = DriveNumber::from_letter(drive_letter) {
                drive
            } else {
                self.current_drive
            };
            (drive, path[2..].to_string())
        } else {
            // Use current drive
            (self.current_drive, path)
        }
    }

    /// Convert DOS path (backslashes) to Unix path (forward slashes)
    fn dos_to_unix_path(dos_path: &str) -> String {
        dos_path.replace('\\', "/")
    }

    /// Convert IO error to DOS error code
    fn map_error(err: io::Error) -> DosError {
        match err.kind() {
            ErrorKind::NotFound => DosError::FileNotFound,
            ErrorKind::AlreadyExists => DosError::AccessDenied,
            ErrorKind::PermissionDenied => DosError::AccessDenied,
            ErrorKind::InvalidInput => DosError::InvalidFunction,
            ErrorKind::WriteZero => DosError::AccessDenied,
            _ => DosError::InvalidFunction,
        }
    }

    /// Resolve a path relative to a drive's current directory
    fn resolve_path(&self, drive: DriveNumber, path: &str) -> String {
        let unix_path = Self::dos_to_unix_path(path);

        if unix_path.starts_with('/') {
            // Absolute path
            unix_path
        } else {
            // Relative path - combine with drive's current directory
            let current_dir = self
                .get_drive(drive)
                .map(|d| d.current_dir.as_str())
                .unwrap_or("/");

            if current_dir == "/" {
                format!("/{}", unix_path)
            } else {
                format!("{}/{}", current_dir, unix_path)
            }
        }
    }

    // === Handle Management ===

    /// Allocate a new file handle
    fn allocate_handle(&mut self) -> u16 {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        if self.next_handle < 3 {
            self.next_handle = 3;
        }
        handle
    }

    /// Check if a handle is valid (opened file)
    pub fn contains_handle(&self, handle: u16) -> bool {
        self.open_files.contains_key(&handle)
    }

    // === File Operations ===

    pub fn file_open(&mut self, filename: &str, access_mode: FileAccess) -> Result<u16, DosError> {
        let (drive, path_part) = self.parse_path(filename);
        let path = self.resolve_path(drive, &path_part);
        log::debug!(
            "DriveManager: Opening file '{}' on drive {} (resolved to '{}')",
            filename,
            drive,
            path
        );

        // CD-ROM: buffer the entire file at open time
        if drive.is_cdrom() {
            let slot = drive.cdrom_slot() as usize;
            let image = self.cdrom_drives[slot]
                .as_ref()
                .ok_or(DosError::InvalidDrive)?;
            let data = image.read_file(&path).map_err(|_| DosError::FileNotFound)?;
            let size = data.len() as u64;
            let handle = self.allocate_handle();
            self.open_files.insert(
                handle,
                FileHandle {
                    drive,
                    path,
                    position: 0,
                    access_mode,
                    cdrom_data: Some(data),
                    cdrom_size: size,
                },
            );
            return Ok(handle);
        }

        // Verify file exists - scope the filesystem access
        {
            let drive_state = self.get_drive_mut(drive).ok_or(DosError::InvalidDrive)?;
            let adapter = drive_state.adapter.as_mut().ok_or(DosError::FileNotFound)?;

            adapter.reset_position();
            let fs = fatfs::FileSystem::new(adapter, fatfs::FsOptions::new()).map_err(|e| {
                log::error!("DriveManager: Failed to create filesystem: {:?}", e);
                Self::map_error(e)
            })?;
            let root_dir = fs.root_dir();

            // Try to open the file to verify it exists
            let _ = root_dir.open_file(&path).map_err(|e| {
                log::warn!("DriveManager: Failed to open file '{}': {:?}", path, e);
                Self::map_error(e)
            })?;
        }

        log::debug!("DriveManager: File '{}' exists and can be opened", path);

        // Allocate handle
        let handle = self.allocate_handle();

        // Store file metadata
        self.open_files.insert(
            handle,
            FileHandle {
                drive,
                path,
                position: 0,
                access_mode,
                cdrom_data: None,
                cdrom_size: 0,
            },
        );

        Ok(handle)
    }

    pub fn file_create(&mut self, filename: &str, _attributes: u8) -> Result<u16, DosError> {
        let (drive, path_part) = self.parse_path(filename);
        let path = self.resolve_path(drive, &path_part);

        // CD-ROM is read-only
        if drive.is_cdrom() {
            return Err(DosError::AccessDenied);
        }

        // Create the file - scope the filesystem access
        {
            let drive_state = self.get_drive_mut(drive).ok_or(DosError::InvalidDrive)?;
            let adapter = drive_state.adapter.as_mut().ok_or(DosError::FileNotFound)?;

            adapter.reset_position();
            let fs = fatfs::FileSystem::new(adapter, fatfs::FsOptions::new())
                .map_err(Self::map_error)?;
            let root_dir = fs.root_dir();

            // Create file (truncates if exists)
            root_dir.create_file(&path).map_err(Self::map_error)?;
        }

        // Allocate handle
        let handle = self.allocate_handle();

        // Store file metadata with write access
        self.open_files.insert(
            handle,
            FileHandle {
                drive,
                path,
                position: 0,
                access_mode: FileAccess::ReadWrite,
                cdrom_data: None,
                cdrom_size: 0,
            },
        );

        Ok(handle)
    }

    pub fn file_close(&mut self, handle: u16) -> Result<(), DosError> {
        self.open_files
            .remove(&handle)
            .ok_or(DosError::InvalidHandle)?;
        Ok(())
    }

    pub fn file_read(&mut self, handle: u16, max_bytes: u16) -> Result<Vec<u8>, DosError> {
        // Get file info first, then release borrow
        let (drive, path, position, is_cdrom) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(DosError::InvalidHandle)?;
            (
                file_handle.drive,
                file_handle.path.clone(),
                file_handle.position,
                file_handle.cdrom_data.is_some(),
            )
        };

        // CD-ROM: serve from buffered data
        if is_cdrom {
            let (result, bytes_read) = {
                let file_handle = self.open_files.get(&handle).unwrap();
                let data = file_handle.cdrom_data.as_ref().unwrap();
                let pos = position as usize;
                let end = (pos + max_bytes as usize).min(data.len());
                let result = if pos >= data.len() {
                    Vec::new()
                } else {
                    data[pos..end].to_vec()
                };
                let bytes_read = result.len();
                (result, bytes_read)
            };
            let file_handle = self.open_files.get_mut(&handle).unwrap();
            file_handle.position += bytes_read as u64;
            return Ok(result);
        }

        // Read from filesystem - scope the filesystem access
        let (buffer, bytes_read) = {
            let drive_state = self.get_drive_mut(drive).ok_or(DosError::InvalidDrive)?;
            let adapter = drive_state.adapter.as_mut().ok_or(DosError::FileNotFound)?;

            adapter.reset_position();
            let fs = fatfs::FileSystem::new(adapter, fatfs::FsOptions::new())
                .map_err(Self::map_error)?;
            let root_dir = fs.root_dir();
            let mut file = root_dir.open_file(&path).map_err(Self::map_error)?;

            // Seek to current position
            file.seek(SeekFrom::Start(position))
                .map_err(|_| DosError::InvalidFunction)?;

            // Read data
            let mut buffer = vec![0u8; max_bytes as usize];
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|_| DosError::InvalidFunction)?;

            buffer.truncate(bytes_read);
            (buffer, bytes_read)
        };

        // Update position
        let file_handle = self.open_files.get_mut(&handle).unwrap();
        file_handle.position += bytes_read as u64;

        Ok(buffer)
    }

    pub fn file_write(&mut self, handle: u16, data: &[u8]) -> Result<u16, DosError> {
        // Get file info first, then release borrow
        let (drive, path, position) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(DosError::InvalidHandle)?;
            // CD-ROM is read-only
            if file_handle.drive.is_cdrom() {
                return Err(DosError::AccessDenied);
            }
            (
                file_handle.drive,
                file_handle.path.clone(),
                file_handle.position,
            )
        };

        // Write to filesystem - scope the filesystem access
        let bytes_written = {
            let drive_state = self.get_drive_mut(drive).ok_or(DosError::InvalidDrive)?;
            let adapter = drive_state.adapter.as_mut().ok_or(DosError::FileNotFound)?;

            adapter.reset_position();
            let fs = fatfs::FileSystem::new(adapter, fatfs::FsOptions::new())
                .map_err(Self::map_error)?;
            let root_dir = fs.root_dir();
            let mut file = root_dir.open_file(&path).map_err(Self::map_error)?;

            // Seek to current position
            file.seek(SeekFrom::Start(position))
                .map_err(|_| DosError::InvalidFunction)?;

            // Write data (write_all ensures all bytes are written, not just up to buffer size)
            file.write_all(data)
                .map_err(|_| DosError::InvalidFunction)?;
            data.len()
        };

        // Update position
        let file_handle = self.open_files.get_mut(&handle).unwrap();
        file_handle.position += bytes_written as u64;

        Ok(bytes_written as u16)
    }

    pub fn file_seek(
        &mut self,
        handle: u16,
        offset: i32,
        method: SeekMethod,
    ) -> Result<u32, DosError> {
        // Get file info first
        let (drive, path, current_position, cdrom_size_opt) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(DosError::InvalidHandle)?;
            (
                file_handle.drive,
                file_handle.path.clone(),
                file_handle.position,
                if file_handle.cdrom_data.is_some() {
                    Some(file_handle.cdrom_size)
                } else {
                    None
                },
            )
        };

        // Calculate new position
        let new_position = if let Some(cdrom_size) = cdrom_size_opt {
            // CD-ROM: use buffered size for FromEnd seeks
            match method {
                SeekMethod::FromStart => offset as i64,
                SeekMethod::FromCurrent => current_position as i64 + offset as i64,
                SeekMethod::FromEnd => cdrom_size as i64 + offset as i64,
            }
        } else {
            match method {
                SeekMethod::FromStart => offset as i64,
                SeekMethod::FromCurrent => current_position as i64 + offset as i64,
                SeekMethod::FromEnd => {
                    // Need to get file size by seeking to end
                    let drive_state = self.get_drive_mut(drive).ok_or(DosError::InvalidDrive)?;
                    let adapter = drive_state.adapter.as_mut().ok_or(DosError::FileNotFound)?;

                    adapter.reset_position();
                    let fs = fatfs::FileSystem::new(adapter, fatfs::FsOptions::new())
                        .map_err(Self::map_error)?;
                    let root_dir = fs.root_dir();
                    let mut file = root_dir.open_file(&path).map_err(Self::map_error)?;
                    let size =
                        file.seek(SeekFrom::End(0))
                            .map_err(|_| DosError::InvalidFunction)? as i64;
                    size + offset as i64
                }
            }
        };

        if new_position < 0 {
            return Err(DosError::InvalidFunction);
        }

        // Update position
        let file_handle = self.open_files.get_mut(&handle).unwrap();
        file_handle.position = new_position as u64;
        Ok(new_position as u32)
    }

    pub fn file_duplicate(&mut self, handle: u16) -> Result<u16, DosError> {
        // Get file info first, then release borrow
        let (drive, path, position, access_mode, cdrom_data, cdrom_size) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(DosError::InvalidHandle)?;
            (
                file_handle.drive,
                file_handle.path.clone(),
                file_handle.position,
                file_handle.access_mode,
                file_handle.cdrom_data.clone(),
                file_handle.cdrom_size,
            )
        };

        // Create new handle with same metadata
        let new_handle = self.allocate_handle();

        self.open_files.insert(
            new_handle,
            FileHandle {
                drive,
                path,
                position,
                access_mode,
                cdrom_data,
                cdrom_size,
            },
        );

        Ok(new_handle)
    }

    // === Directory Operations ===

    pub fn dir_create(&mut self, dirname: &str) -> Result<(), DosError> {
        let (drive, path_part) = self.parse_path(dirname);
        let path = self.resolve_path(drive, &path_part);

        if drive.is_cdrom() {
            return Err(DosError::AccessDenied);
        }

        let drive_state = self.get_drive_mut(drive).ok_or(DosError::InvalidDrive)?;
        let adapter = drive_state.adapter.as_mut().ok_or(DosError::FileNotFound)?;

        adapter.reset_position();
        let fs =
            fatfs::FileSystem::new(adapter, fatfs::FsOptions::new()).map_err(Self::map_error)?;
        let root_dir = fs.root_dir();

        root_dir.create_dir(&path).map_err(Self::map_error)?;

        Ok(())
    }

    pub fn dir_remove(&mut self, dirname: &str) -> Result<(), DosError> {
        let (drive, path_part) = self.parse_path(dirname);
        let path = self.resolve_path(drive, &path_part);

        if drive.is_cdrom() {
            return Err(DosError::AccessDenied);
        }

        let drive_state = self.get_drive_mut(drive).ok_or(DosError::InvalidDrive)?;
        let adapter = drive_state.adapter.as_mut().ok_or(DosError::FileNotFound)?;

        adapter.reset_position();
        let fs =
            fatfs::FileSystem::new(adapter, fatfs::FsOptions::new()).map_err(Self::map_error)?;
        let root_dir = fs.root_dir();

        root_dir.remove(&path).map_err(Self::map_error)?;

        Ok(())
    }

    pub fn file_delete(&mut self, filename: &str) -> Result<(), DosError> {
        let (drive, path_part) = self.parse_path(filename);
        let path = self.resolve_path(drive, &path_part);

        if drive.is_cdrom() {
            return Err(DosError::AccessDenied);
        }

        let drive_state = self.get_drive_mut(drive).ok_or(DosError::InvalidDrive)?;
        let adapter = drive_state.adapter.as_mut().ok_or(DosError::FileNotFound)?;

        adapter.reset_position();
        let fs =
            fatfs::FileSystem::new(adapter, fatfs::FsOptions::new()).map_err(Self::map_error)?;
        let root_dir = fs.root_dir();

        root_dir.remove(&path).map_err(Self::map_error)?;

        Ok(())
    }

    pub fn dir_change(&mut self, dirname: &str) -> Result<(), DosError> {
        let (drive, path_part) = self.parse_path(dirname);
        let path = self.resolve_path(drive, &path_part);

        if drive.is_cdrom() {
            return Err(DosError::AccessDenied);
        }

        // First verify directory exists
        {
            let drive_state = self.get_drive_mut(drive).ok_or(DosError::InvalidDrive)?;
            let adapter = drive_state.adapter.as_mut().ok_or(DosError::FileNotFound)?;

            adapter.reset_position();
            let fs = fatfs::FileSystem::new(adapter, fatfs::FsOptions::new())
                .map_err(Self::map_error)?;
            let root_dir = fs.root_dir();

            // Try to open as directory
            let _dir = root_dir.open_dir(&path).map_err(Self::map_error)?;
        }

        // Update current directory for this drive
        let drive_state = self.get_drive_mut(drive).unwrap();
        drive_state.current_dir = path;

        Ok(())
    }

    pub fn find_first(
        &mut self,
        pattern: &str,
        attributes: u8,
    ) -> Result<(usize, FindData), DosError> {
        let (drive, path_pattern) = self.parse_path(pattern);

        // Extract directory path and filename pattern
        // e.g., "/ZORK1/*.*" -> path="/ZORK1", filename_pattern="*.*"
        let (mut path, filename_pattern) = if let Some(last_slash) = path_pattern.rfind('/') {
            let dir = &path_pattern[..last_slash];
            let file = &path_pattern[last_slash + 1..];
            (
                if dir.is_empty() {
                    "/".to_string()
                } else {
                    dir.to_string()
                },
                file,
            )
        } else {
            // No slash, use current directory
            (self.resolve_path(drive, "."), path_pattern.as_str())
        };

        // Normalize "/." to "/" since fatfs doesn't understand current directory notation
        if path == "/." {
            path = String::from("/");
        }

        // CD-ROM: list directory using ISO 9660
        if drive.is_cdrom() {
            let slot = drive.cdrom_slot() as usize;
            let iso_entries = {
                let image = self.cdrom_drives[slot]
                    .as_ref()
                    .ok_or(DosError::InvalidDrive)?;
                image
                    .list_directory(&path)
                    .map_err(|_| DosError::FileNotFound)?
            };
            let entries: Vec<FindData> = iso_entries
                .iter()
                .filter(|e| Self::matches_pattern(&e.name, filename_pattern))
                .map(|e| Self::iso_entry_to_find_data(e))
                .collect();
            if entries.is_empty() {
                return Err(DosError::NoMoreFiles);
            }
            let search_id = self.next_search_id;
            self.next_search_id += 1;
            let first_entry = entries[0].clone();
            self.searches.insert(
                search_id,
                SearchState {
                    drive,
                    entries,
                    index: 1,
                },
            );
            return Ok((search_id, first_entry));
        }

        // Collect entries in a scoped block so fs is dropped before we access self.next_search_id
        let entries = {
            let drive_state = self.get_drive_mut(drive).ok_or(DosError::InvalidDrive)?;
            let adapter = drive_state.adapter.as_mut().ok_or(DosError::FileNotFound)?;

            adapter.reset_position();
            let fs = fatfs::FileSystem::new(adapter, fatfs::FsOptions::new())
                .map_err(Self::map_error)?;
            let root_dir = fs.root_dir();
            let dir = if path == "/" {
                root_dir
            } else {
                root_dir.open_dir(&path).map_err(Self::map_error)?
            };

            // Collect all entries
            let mut entries = Vec::new();
            for entry in dir.iter() {
                let entry = entry.map_err(Self::map_error)?;
                let name = entry.file_name();

                // Skip "." and ".."
                if name == "." || name == ".." {
                    continue;
                }

                // Check if matches pattern (simple wildcard matching)
                if !Self::matches_pattern(&name, filename_pattern) {
                    continue;
                }

                // Convert to FindData
                let find_data = Self::entry_to_find_data(&entry)?;

                // Filter by attributes (DOS behavior: always include normal files, plus any requested special types)
                // Special attributes: 0x02=Hidden, 0x04=System, 0x10=Directory
                const SPECIAL_ATTRS: u8 = 0x02 | 0x04 | 0x10;
                let is_special = (find_data.attributes & SPECIAL_ATTRS) != 0;
                let matches_filter = (find_data.attributes & attributes) != 0;

                if attributes != 0 && is_special && !matches_filter {
                    continue;
                }

                entries.push(find_data);
            }
            entries
        };

        if entries.is_empty() {
            return Err(DosError::NoMoreFiles);
        }

        // Store search state
        let search_id = self.next_search_id;
        self.next_search_id += 1;

        let first_entry = entries[0].clone();

        self.searches.insert(
            search_id,
            SearchState {
                drive,
                entries,
                index: 1,
            },
        );

        Ok((search_id, first_entry))
    }

    pub fn find_next(&mut self, search_id: usize) -> Result<FindData, DosError> {
        let search = self
            .searches
            .get_mut(&search_id)
            .ok_or(DosError::NoMoreFiles)?;

        if search.index >= search.entries.len() {
            return Err(DosError::NoMoreFiles);
        }

        let entry = search.entries[search.index].clone();
        search.index += 1;

        Ok(entry)
    }

    /// Helper: Simple wildcard pattern matching
    fn matches_pattern(name: &str, pattern: &str) -> bool {
        // Convert to uppercase for case-insensitive matching
        let name = name.to_uppercase();
        let pattern = pattern.to_uppercase();

        // Strip drive letter if present
        let pattern = if pattern.len() >= 2 && pattern.chars().nth(1) == Some(':') {
            &pattern[2..]
        } else {
            &pattern
        };

        // Handle simple patterns
        if pattern == "*.*" || pattern == "*" {
            return true;
        }

        // Simple wildcard matching
        name.contains(&pattern.replace('*', ""))
    }

    /// Helper: Convert fatfs directory entry to FindData
    fn entry_to_find_data<T>(entry: &fatfs::DirEntry<'_, T>) -> Result<FindData, DosError>
    where
        T: fatfs::ReadWriteSeek,
    {
        let name = entry.file_name();
        let is_dir = entry.is_dir();
        let size = if is_dir {
            0
        } else {
            let mut file = entry.to_file();
            file.seek(SeekFrom::End(0)).unwrap_or(0) as u32
        };

        let modified = entry.modified();
        let time = modified.time;
        let date = modified.date;

        let dos_time = (time.hour << 11) | (time.min << 5) | (time.sec / 2);
        let dos_date = ((date.year - 1980) << 9) | (date.month << 5) | date.day;

        let mut attributes = 0u8;
        if is_dir {
            attributes |= 0x10; // DIRECTORY
        }

        Ok(FindData {
            attributes,
            time: dos_time,
            date: dos_date,
            size,
            filename: name,
        })
    }

    // === Disk Operations (INT 13h) ===

    pub fn disk_read_sectors(
        &self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, DiskError> {
        let drive_state = self.get_drive(drive).ok_or(DiskError::DriveNotReady)?;

        // Use raw_adapter for INT 13h if available (partitioned hard drives)
        // This allows reading the MBR at sector 0
        let adapter = if let Some(ref raw) = drive_state.raw_adapter {
            raw
        } else {
            drive_state
                .adapter
                .as_ref()
                .ok_or(DiskError::DriveNotReady)?
        };

        // Get disk geometry for proper C/H/S wrapping
        let geometry = adapter.disk().geometry();
        let sectors_per_track = geometry.sectors_per_track;
        let heads = geometry.heads;

        let mut current_cylinder = cylinder as u16;
        let mut current_head = head as u16;
        let mut current_sector = sector as u16;

        let mut result = Vec::new();

        for _ in 0..count {
            let sector_data = adapter
                .disk()
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

    pub fn disk_write_sectors(
        &mut self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
        data: &[u8],
    ) -> Result<u8, DiskError> {
        let drive_state = self.get_drive_mut(drive).ok_or(DiskError::DriveNotReady)?;

        // Use raw_adapter for INT 13h if available (partitioned hard drives)
        let use_raw = drive_state.raw_adapter.is_some();
        let adapter = if use_raw {
            drive_state.raw_adapter.as_mut().unwrap()
        } else {
            drive_state
                .adapter
                .as_mut()
                .ok_or(DiskError::DriveNotReady)?
        };

        if adapter.disk().is_read_only() {
            return Err(DiskError::WriteProtected);
        }

        // Get disk geometry for proper C/H/S wrapping
        let geometry = adapter.disk().geometry();
        let sectors_per_track = geometry.sectors_per_track;
        let heads = geometry.heads;

        let mut current_cylinder = cylinder as u16;
        let mut current_head = head as u16;
        let mut current_sector = sector as u16;

        let mut written = 0;
        for i in 0..count {
            let offset = i as usize * SECTOR_SIZE;
            if offset + SECTOR_SIZE > data.len() {
                break;
            }

            let mut sector_data = [0u8; SECTOR_SIZE];
            sector_data.copy_from_slice(&data[offset..offset + SECTOR_SIZE]);

            adapter
                .disk_mut()
                .write_sector_chs(current_cylinder, current_head, current_sector, &sector_data)
                .map_err(|_| DiskError::SectorNotFound)?;

            written += 1;

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

        Ok(written)
    }

    pub fn disk_reset(&mut self, _drive: DriveNumber) -> bool {
        true // Always succeeds in our implementation
    }

    pub fn disk_get_params(&self, drive: DriveNumber) -> Result<DriveParams, DiskError> {
        let drive_state = self.get_drive(drive).ok_or(DiskError::DriveNotReady)?;

        // Use raw_adapter for INT 13h if available (partitioned hard drives)
        let adapter = if let Some(ref raw) = drive_state.raw_adapter {
            raw
        } else {
            drive_state
                .adapter
                .as_ref()
                .ok_or(DiskError::DriveNotReady)?
        };

        let geometry = adapter.disk().geometry();

        // Count drives of this type
        let drive_count = if drive.is_floppy() {
            // Count floppy drives with disks
            self.floppy_drives.iter().filter(|d| d.has_disk()).count() as u8
        } else {
            self.hard_drives.len() as u8
        };

        Ok(DriveParams {
            max_cylinder: (geometry.cylinders - 1).min(255) as u8,
            max_head: (geometry.heads - 1).min(255) as u8,
            max_sector: geometry.sectors_per_track.min(255) as u8,
            drive_count,
        })
    }

    pub fn disk_get_type(&self, drive: DriveNumber) -> Result<(u8, u32), DiskError> {
        let drive_state = self.get_drive(drive).ok_or(DiskError::DriveNotReady)?;

        // Use raw_adapter for INT 13h if available (partitioned hard drives)
        let adapter = if let Some(ref raw) = drive_state.raw_adapter {
            raw
        } else {
            drive_state
                .adapter
                .as_ref()
                .ok_or(DiskError::DriveNotReady)?
        };

        let geometry = adapter.disk().geometry();
        let sectors = geometry.total_sectors() as u32;

        // Drive type:
        // 0x00 = not present
        // 0x01 = floppy without change-line support
        // 0x02 = floppy with change-line support
        // 0x03 = fixed disk (hard drive)
        let drive_type = if drive.is_floppy() {
            0x02 // Floppy with change-line support
        } else {
            0x03 // Fixed disk
        };

        Ok((drive_type, sectors))
    }

    pub fn disk_detect_change(&mut self, drive: DriveNumber) -> Result<bool, DiskError> {
        let drive_state = self.get_drive_mut(drive).ok_or(DiskError::DriveNotReady)?;

        if !drive_state.removable {
            // Fixed disks never change
            return Ok(false);
        }

        if !drive_state.has_disk() {
            return Err(DiskError::DriveNotReady);
        }

        // Return and clear the change flag
        let changed = drive_state.disk_changed;
        drive_state.disk_changed = false;
        Ok(changed)
    }

    pub fn disk_format_track(
        &mut self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sectors_per_track: u8,
    ) -> Result<(), DiskError> {
        let drive_state = self.get_drive_mut(drive).ok_or(DiskError::DriveNotReady)?;

        // Use raw_adapter for INT 13h if available (partitioned hard drives)
        let use_raw = drive_state.raw_adapter.is_some();
        let adapter = if use_raw {
            drive_state.raw_adapter.as_mut().unwrap()
        } else {
            drive_state
                .adapter
                .as_mut()
                .ok_or(DiskError::DriveNotReady)?
        };

        if adapter.disk().is_read_only() {
            return Err(DiskError::WriteProtected);
        }

        // Format track by writing zeros to each sector
        let zero_sector = [0u8; SECTOR_SIZE];

        for sector in 1..=sectors_per_track {
            adapter
                .disk_mut()
                .write_sector_chs(cylinder as u16, head as u16, sector as u16, &zero_sector)
                .map_err(|e| {
                    log::warn!(
                        "disk_format_track: Failed to write sector C={} H={} S={}: {:?}",
                        cylinder,
                        head,
                        sector,
                        e
                    );
                    DiskError::BadSector
                })?;
        }

        log::debug!(
            "disk_format_track: Formatted track C={} H={} with {} sectors",
            cylinder,
            head,
            sectors_per_track
        );

        Ok(())
    }

    /// Read sectors using logical sector addressing (INT 25h)
    /// start_sector: starting logical sector number
    /// count: number of sectors to read
    pub fn disk_read_sectors_lba(
        &self,
        drive: DriveNumber,
        start_sector: u32,
        count: u16,
    ) -> Result<Vec<u8>, DiskError> {
        let drive_state = self.get_drive(drive).ok_or(DiskError::DriveNotReady)?;
        let adapter = drive_state
            .adapter
            .as_ref()
            .ok_or(DiskError::DriveNotReady)?;

        let mut result = Vec::with_capacity(count as usize * SECTOR_SIZE);

        for i in 0..count {
            let lba = start_sector as usize + i as usize;
            let sector_data = adapter
                .disk()
                .read_sector_lba(lba)
                .map_err(|_| DiskError::SectorNotFound)?;
            result.extend_from_slice(&sector_data);
        }

        Ok(result)
    }

    /// Get immutable access to a floppy disk (for saving)
    pub fn get_floppy_disk(&self, drive: DriveNumber) -> Option<&dyn DiskController> {
        if !drive.is_floppy() {
            return None;
        }
        self.floppy_drives[drive.to_floppy_index()]
            .adapter
            .as_ref()
            .map(|a| a.disk())
    }

    /// Get immutable access to a hard drive (for saving)
    pub fn get_hard_drive_disk(&self, drive: DriveNumber) -> Option<&dyn DiskController> {
        self.hard_drives
            .get(drive.to_hard_drive_index())
            .and_then(|d| {
                // Use raw_adapter for partitioned drives (includes MBR)
                // Fall back to regular adapter for non-partitioned drives
                if let Some(ref raw) = d.raw_adapter {
                    Some(raw.disk())
                } else {
                    d.adapter.as_ref().map(|a| a.disk())
                }
            })
    }

    /// Get the number of hard drives
    pub fn hard_drive_count(&self) -> usize {
        self.hard_drives.len()
    }
}
