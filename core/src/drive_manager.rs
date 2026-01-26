//! Multi-drive management for the 8086 emulator
//!
//! Supports:
//! - 2 floppy drive slots (A: = 0x00, B: = 0x01) - can be empty or contain disk
//! - Multiple hard drives (C: = 0x80, D: = 0x81, etc.) - always present once added
//! - Per-drive current directory tracking
//! - Disk change detection flags for floppies

use crate::cpu::bios::{DriveParams, FindData, SeekMethod, disk_errors, dos_errors};
use crate::disk::{DiskController, SECTOR_SIZE};
use std::collections::HashMap;
use std::io::{self, Error, ErrorKind, Read, Seek, SeekFrom, Write};

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

/// Adapter to make DiskController work with fatfs crate's ReadWriteSeek trait
pub struct DiskAdapter<D: DiskController> {
    disk: D,
    position: u64,
}

impl<D: DiskController> DiskAdapter<D> {
    pub fn new(disk: D) -> Self {
        Self { disk, position: 0 }
    }

    /// Get the total size of the disk in bytes
    fn size(&self) -> u64 {
        self.disk.size() as u64
    }

    /// Get mutable access to the underlying disk (for pass-through operations)
    pub fn disk_mut(&mut self) -> &mut D {
        &mut self.disk
    }

    /// Get immutable access to the underlying disk
    pub fn disk(&self) -> &D {
        &self.disk
    }

    /// Reset adapter position to 0 (required before fatfs FileSystem::new())
    pub fn reset_position(&mut self) {
        self.position = 0;
    }

    /// Consume adapter and return the underlying disk
    pub fn into_disk(self) -> D {
        self.disk
    }
}

impl<D: DiskController> Read for DiskAdapter<D> {
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

impl<D: DiskController> Write for DiskAdapter<D> {
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

impl<D: DiskController> Seek for DiskAdapter<D> {
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
pub struct DriveState<D: DiskController> {
    /// The disk adapter (None for empty floppy slot)
    adapter: Option<DiskAdapter<D>>,
    /// Current working directory for this drive (Unix-style path with forward slashes)
    current_dir: String,
    /// Disk change flag (true if disk was changed since last check)
    disk_changed: bool,
    /// Whether this is a removable drive (floppy)
    removable: bool,
}

impl<D: DiskController> DriveState<D> {
    /// Create a new drive state with a disk
    pub fn new_with_disk(disk: D, removable: bool) -> Self {
        Self {
            adapter: Some(DiskAdapter::new(disk)),
            current_dir: String::from("/"),
            disk_changed: false,
            removable,
        }
    }

    /// Create an empty drive slot (for removable drives)
    pub fn empty() -> Self {
        Self {
            adapter: None,
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
    drive: u8,
    /// Path on that drive
    path: String,
    /// Current position in file
    position: u64,
    /// Access mode (read/write/both)
    access_mode: u8,
}

/// Search state for directory iteration
struct SearchState {
    /// Which drive this search is on
    drive: u8,
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
/// - etc.
pub struct DriveManager<D: DiskController> {
    /// Floppy drive slots (A: = 0, B: = 1)
    floppy_drives: [DriveState<D>; 2],

    /// Hard drives (C: = 0x80, D: = 0x81, etc.)
    hard_drives: Vec<DriveState<D>>,

    /// Current default drive (0 = A:, 1 = B:, 0x80 = C:, etc.)
    current_drive: u8,

    /// Open file handles: maps handle -> file metadata
    open_files: HashMap<u16, FileHandle>,

    /// Search states for find first/next
    searches: HashMap<usize, SearchState>,

    /// Next file handle to allocate (global across all drives)
    next_handle: u16,

    /// Next search ID
    next_search_id: usize,
}

impl<D: DiskController> Default for DriveManager<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D: DiskController> DriveManager<D> {
    /// Create a new drive manager with empty floppy slots and no hard drives
    pub fn new() -> Self {
        Self {
            floppy_drives: [DriveState::empty(), DriveState::empty()],
            hard_drives: Vec::new(),
            current_drive: 0,
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

    // === Floppy Management ===

    /// Insert a floppy disk into a slot (0 = A:, 1 = B:)
    pub fn insert_floppy(&mut self, slot: u8, disk: D) -> Result<(), String> {
        if slot > 1 {
            return Err(format!("Invalid floppy slot: {} (must be 0 or 1)", slot));
        }

        // Close any open files on this drive first
        self.close_files_on_drive(slot);

        self.floppy_drives[slot as usize] = DriveState::new_with_disk(disk, true);
        self.floppy_drives[slot as usize].disk_changed = true; // Mark as changed

        Ok(())
    }

    /// Eject a floppy disk from a slot, returning the disk if present
    pub fn eject_floppy(&mut self, slot: u8) -> Result<Option<D>, String> {
        if slot > 1 {
            return Err(format!("Invalid floppy slot: {} (must be 0 or 1)", slot));
        }

        // Close any open files on this drive
        self.close_files_on_drive(slot);

        let old_state =
            std::mem::replace(&mut self.floppy_drives[slot as usize], DriveState::empty());

        // Return the disk if there was one
        Ok(old_state.adapter.map(|a| a.into_disk()))
    }

    // === Hard Drive Management ===

    /// Add a hard drive (returns the assigned drive number: 0x80, 0x81, etc.)
    pub fn add_hard_drive(&mut self, disk: D) -> u8 {
        let drive_number = 0x80 + self.hard_drives.len() as u8;
        self.hard_drives
            .push(DriveState::new_with_disk(disk, false));
        drive_number
    }

    // === Drive Access ===

    /// Get mutable reference to a drive state
    fn get_drive_mut(&mut self, drive: u8) -> Option<&mut DriveState<D>> {
        if drive < 0x80 {
            // Floppy drive
            if drive < 2 {
                Some(&mut self.floppy_drives[drive as usize])
            } else {
                None
            }
        } else {
            // Hard drive
            let index = (drive - 0x80) as usize;
            self.hard_drives.get_mut(index)
        }
    }

    /// Get immutable reference to a drive state
    fn get_drive(&self, drive: u8) -> Option<&DriveState<D>> {
        if drive < 0x80 {
            if drive < 2 {
                Some(&self.floppy_drives[drive as usize])
            } else {
                None
            }
        } else {
            let index = (drive - 0x80) as usize;
            self.hard_drives.get(index)
        }
    }

    /// Close all open files on a specific drive
    fn close_files_on_drive(&mut self, drive: u8) {
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
    pub fn get_current_drive(&self) -> u8 {
        self.current_drive
    }

    /// Set the current default drive
    /// Returns the total number of logical drives on success
    pub fn set_current_drive(&mut self, drive: u8) -> Result<u8, u8> {
        // Verify drive exists
        if self.get_drive(drive).is_some() {
            self.current_drive = drive;
            // Return number of logical drives (floppies + hard drives)
            let floppy_count = 2u8; // Always report 2 floppy slots
            let hd_count = self.hard_drives.len() as u8;
            Ok(floppy_count + hd_count)
        } else {
            Err(dos_errors::INVALID_DRIVE)
        }
    }

    /// Get current directory for a drive
    /// drive: 0 = use current default drive, 1 = A:, 2 = B:, 3 = C:, etc.
    pub fn get_current_dir(&self, drive: u8) -> Result<String, u8> {
        let target_drive = if drive == 0 {
            self.current_drive
        } else {
            // DOS uses 1=A:, 2=B:, 3=C:, etc. for this function
            Self::dos_drive_to_internal(drive - 1)
        };

        self.get_drive(target_drive)
            .map(|d| {
                // Convert Unix path back to DOS path
                d.current_dir.replace('/', "\\")
            })
            .ok_or(dos_errors::INVALID_DRIVE)
    }

    /// Convert DOS drive number (0=A, 1=B, 2=C, ...) to internal (0x00, 0x01, 0x80, ...)
    fn dos_drive_to_internal(dos_drive: u8) -> u8 {
        if dos_drive < 2 {
            dos_drive // A: and B: are 0x00 and 0x01
        } else {
            0x80 + (dos_drive - 2) // C: onwards are 0x80+
        }
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
    fn parse_path(&self, path: &str) -> (u8, String) {
        let path = path.replace('\\', "/");

        if path.len() >= 2 && path.chars().nth(1) == Some(':') {
            // Has drive letter
            let drive_letter = path.chars().next().unwrap().to_ascii_uppercase();
            let drive = match drive_letter {
                'A' => 0x00,
                'B' => 0x01,
                'C'..='Z' => 0x80 + (drive_letter as u8 - b'C'),
                _ => self.current_drive,
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
    fn map_error(err: io::Error) -> u8 {
        match err.kind() {
            ErrorKind::NotFound => dos_errors::FILE_NOT_FOUND,
            ErrorKind::AlreadyExists => dos_errors::ACCESS_DENIED,
            ErrorKind::PermissionDenied => dos_errors::ACCESS_DENIED,
            ErrorKind::InvalidInput => dos_errors::INVALID_FUNCTION,
            ErrorKind::WriteZero => dos_errors::ACCESS_DENIED,
            _ => dos_errors::INVALID_FUNCTION,
        }
    }

    /// Resolve a path relative to a drive's current directory
    fn resolve_path(&self, drive: u8, path: &str) -> String {
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

    pub fn file_open(&mut self, filename: &str, access_mode: u8) -> Result<u16, u8> {
        let (drive, path_part) = self.parse_path(filename);
        let path = self.resolve_path(drive, &path_part);
        log::debug!(
            "DriveManager: Opening file '{}' on drive {:02X} (resolved to '{}')",
            filename,
            drive,
            path
        );

        // Verify file exists - scope the filesystem access
        {
            let drive_state = self.get_drive_mut(drive).ok_or(dos_errors::INVALID_DRIVE)?;
            let adapter = drive_state
                .adapter
                .as_mut()
                .ok_or(disk_errors::DRIVE_NOT_READY)?;

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
            },
        );

        Ok(handle)
    }

    pub fn file_create(&mut self, filename: &str, _attributes: u8) -> Result<u16, u8> {
        let (drive, path_part) = self.parse_path(filename);
        let path = self.resolve_path(drive, &path_part);

        // Create the file - scope the filesystem access
        {
            let drive_state = self.get_drive_mut(drive).ok_or(dos_errors::INVALID_DRIVE)?;
            let adapter = drive_state
                .adapter
                .as_mut()
                .ok_or(disk_errors::DRIVE_NOT_READY)?;

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
                access_mode: 2, // Read-write access
            },
        );

        Ok(handle)
    }

    pub fn file_close(&mut self, handle: u16) -> Result<(), u8> {
        self.open_files
            .remove(&handle)
            .ok_or(dos_errors::INVALID_HANDLE)?;
        Ok(())
    }

    pub fn file_read(&mut self, handle: u16, max_bytes: u16) -> Result<Vec<u8>, u8> {
        // Get file info first, then release borrow
        let (drive, path, position) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(dos_errors::INVALID_HANDLE)?;
            (
                file_handle.drive,
                file_handle.path.clone(),
                file_handle.position,
            )
        };

        // Read from filesystem - scope the filesystem access
        let (buffer, bytes_read) = {
            let drive_state = self.get_drive_mut(drive).ok_or(dos_errors::INVALID_DRIVE)?;
            let adapter = drive_state
                .adapter
                .as_mut()
                .ok_or(disk_errors::DRIVE_NOT_READY)?;

            adapter.reset_position();
            let fs = fatfs::FileSystem::new(adapter, fatfs::FsOptions::new())
                .map_err(Self::map_error)?;
            let root_dir = fs.root_dir();
            let mut file = root_dir.open_file(&path).map_err(Self::map_error)?;

            // Seek to current position
            file.seek(SeekFrom::Start(position))
                .map_err(|_| dos_errors::INVALID_FUNCTION)?;

            // Read data
            let mut buffer = vec![0u8; max_bytes as usize];
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|_| dos_errors::INVALID_FUNCTION)?;

            buffer.truncate(bytes_read);
            (buffer, bytes_read)
        };

        // Update position
        let file_handle = self.open_files.get_mut(&handle).unwrap();
        file_handle.position += bytes_read as u64;

        Ok(buffer)
    }

    pub fn file_write(&mut self, handle: u16, data: &[u8]) -> Result<u16, u8> {
        // Get file info first, then release borrow
        let (drive, path, position) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(dos_errors::INVALID_HANDLE)?;
            (
                file_handle.drive,
                file_handle.path.clone(),
                file_handle.position,
            )
        };

        // Write to filesystem - scope the filesystem access
        let bytes_written = {
            let drive_state = self.get_drive_mut(drive).ok_or(dos_errors::INVALID_DRIVE)?;
            let adapter = drive_state
                .adapter
                .as_mut()
                .ok_or(disk_errors::DRIVE_NOT_READY)?;

            adapter.reset_position();
            let fs = fatfs::FileSystem::new(adapter, fatfs::FsOptions::new())
                .map_err(Self::map_error)?;
            let root_dir = fs.root_dir();
            let mut file = root_dir.open_file(&path).map_err(Self::map_error)?;

            // Seek to current position
            file.seek(SeekFrom::Start(position))
                .map_err(|_| dos_errors::INVALID_FUNCTION)?;

            // Write data
            file.write(data).map_err(|_| dos_errors::INVALID_FUNCTION)?
        };

        // Update position
        let file_handle = self.open_files.get_mut(&handle).unwrap();
        file_handle.position += bytes_written as u64;

        Ok(bytes_written as u16)
    }

    pub fn file_seek(&mut self, handle: u16, offset: i32, method: SeekMethod) -> Result<u32, u8> {
        // Get file info first
        let (drive, path, current_position) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(dos_errors::INVALID_HANDLE)?;
            (
                file_handle.drive,
                file_handle.path.clone(),
                file_handle.position,
            )
        };

        // Calculate new position
        let new_position = match method {
            SeekMethod::FromStart => offset as i64,
            SeekMethod::FromCurrent => current_position as i64 + offset as i64,
            SeekMethod::FromEnd => {
                // Need to get file size by seeking to end
                let drive_state = self.get_drive_mut(drive).ok_or(dos_errors::INVALID_DRIVE)?;
                let adapter = drive_state
                    .adapter
                    .as_mut()
                    .ok_or(disk_errors::DRIVE_NOT_READY)?;

                adapter.reset_position();
                let fs = fatfs::FileSystem::new(adapter, fatfs::FsOptions::new())
                    .map_err(Self::map_error)?;
                let root_dir = fs.root_dir();
                let mut file = root_dir.open_file(&path).map_err(Self::map_error)?;
                let size = file
                    .seek(SeekFrom::End(0))
                    .map_err(|_| dos_errors::INVALID_FUNCTION)? as i64;
                size + offset as i64
            }
        };

        if new_position < 0 {
            return Err(dos_errors::INVALID_FUNCTION);
        }

        // Update position
        let file_handle = self.open_files.get_mut(&handle).unwrap();
        file_handle.position = new_position as u64;
        Ok(new_position as u32)
    }

    pub fn file_duplicate(&mut self, handle: u16) -> Result<u16, u8> {
        // Get file info first, then release borrow
        let (drive, path, position, access_mode) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(dos_errors::INVALID_HANDLE)?;
            (
                file_handle.drive,
                file_handle.path.clone(),
                file_handle.position,
                file_handle.access_mode,
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
            },
        );

        Ok(new_handle)
    }

    // === Directory Operations ===

    pub fn dir_create(&mut self, dirname: &str) -> Result<(), u8> {
        let (drive, path_part) = self.parse_path(dirname);
        let path = self.resolve_path(drive, &path_part);

        let drive_state = self.get_drive_mut(drive).ok_or(dos_errors::INVALID_DRIVE)?;
        let adapter = drive_state
            .adapter
            .as_mut()
            .ok_or(disk_errors::DRIVE_NOT_READY)?;

        adapter.reset_position();
        let fs =
            fatfs::FileSystem::new(adapter, fatfs::FsOptions::new()).map_err(Self::map_error)?;
        let root_dir = fs.root_dir();

        root_dir.create_dir(&path).map_err(Self::map_error)?;

        Ok(())
    }

    pub fn dir_remove(&mut self, dirname: &str) -> Result<(), u8> {
        let (drive, path_part) = self.parse_path(dirname);
        let path = self.resolve_path(drive, &path_part);

        let drive_state = self.get_drive_mut(drive).ok_or(dos_errors::INVALID_DRIVE)?;
        let adapter = drive_state
            .adapter
            .as_mut()
            .ok_or(disk_errors::DRIVE_NOT_READY)?;

        adapter.reset_position();
        let fs =
            fatfs::FileSystem::new(adapter, fatfs::FsOptions::new()).map_err(Self::map_error)?;
        let root_dir = fs.root_dir();

        root_dir.remove(&path).map_err(Self::map_error)?;

        Ok(())
    }

    pub fn dir_change(&mut self, dirname: &str) -> Result<(), u8> {
        let (drive, path_part) = self.parse_path(dirname);
        let path = self.resolve_path(drive, &path_part);

        // First verify directory exists
        {
            let drive_state = self.get_drive_mut(drive).ok_or(dos_errors::INVALID_DRIVE)?;
            let adapter = drive_state
                .adapter
                .as_mut()
                .ok_or(disk_errors::DRIVE_NOT_READY)?;

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

    pub fn find_first(&mut self, pattern: &str, attributes: u8) -> Result<(usize, FindData), u8> {
        let (drive, _) = self.parse_path(pattern);
        let path = self.resolve_path(drive, ".");

        // Collect entries in a scoped block so fs is dropped before we access self.next_search_id
        let entries = {
            let drive_state = self.get_drive_mut(drive).ok_or(dos_errors::INVALID_DRIVE)?;
            let adapter = drive_state
                .adapter
                .as_mut()
                .ok_or(disk_errors::DRIVE_NOT_READY)?;

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
                if !Self::matches_pattern(&name, pattern) {
                    continue;
                }

                // Convert to FindData
                let find_data = Self::entry_to_find_data(&entry)?;

                // Filter by attributes
                if attributes != 0 && (find_data.attributes & attributes) == 0 {
                    continue;
                }

                entries.push(find_data);
            }
            entries
        };

        if entries.is_empty() {
            return Err(dos_errors::NO_MORE_FILES);
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

    pub fn find_next(&mut self, search_id: usize) -> Result<FindData, u8> {
        let search = self
            .searches
            .get_mut(&search_id)
            .ok_or(dos_errors::NO_MORE_FILES)?;

        if search.index >= search.entries.len() {
            return Err(dos_errors::NO_MORE_FILES);
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
    fn entry_to_find_data<T>(entry: &fatfs::DirEntry<'_, T>) -> Result<FindData, u8>
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
        drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, u8> {
        let drive_state = self.get_drive(drive).ok_or(disk_errors::DRIVE_NOT_READY)?;
        let adapter = drive_state
            .adapter
            .as_ref()
            .ok_or(disk_errors::DRIVE_NOT_READY)?;

        let mut result = Vec::new();

        for i in 0..count {
            let sector_data = adapter
                .disk()
                .read_sector_chs(cylinder as u16, head as u16, (sector + i) as u16)
                .map_err(|_| disk_errors::SECTOR_NOT_FOUND)?;
            result.extend_from_slice(&sector_data);
        }

        Ok(result)
    }

    pub fn disk_write_sectors(
        &mut self,
        drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
        data: &[u8],
    ) -> Result<u8, u8> {
        let drive_state = self
            .get_drive_mut(drive)
            .ok_or(disk_errors::DRIVE_NOT_READY)?;
        let adapter = drive_state
            .adapter
            .as_mut()
            .ok_or(disk_errors::DRIVE_NOT_READY)?;

        if adapter.disk().is_read_only() {
            return Err(disk_errors::WRITE_PROTECTED);
        }

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
                .write_sector_chs(
                    cylinder as u16,
                    head as u16,
                    (sector + i) as u16,
                    &sector_data,
                )
                .map_err(|_| disk_errors::SECTOR_NOT_FOUND)?;

            written += 1;
        }

        Ok(written)
    }

    pub fn disk_reset(&mut self, _drive: u8) -> bool {
        true // Always succeeds in our implementation
    }

    pub fn disk_get_params(&self, drive: u8) -> Result<DriveParams, u8> {
        let drive_state = self.get_drive(drive).ok_or(disk_errors::DRIVE_NOT_READY)?;
        let adapter = drive_state
            .adapter
            .as_ref()
            .ok_or(disk_errors::DRIVE_NOT_READY)?;

        let geometry = adapter.disk().geometry();

        // Count drives of this type
        let drive_count = if drive < 0x80 {
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

    pub fn disk_get_type(&self, drive: u8) -> Result<(u8, u32), u8> {
        let drive_state = self.get_drive(drive).ok_or(disk_errors::DRIVE_NOT_READY)?;
        let adapter = drive_state
            .adapter
            .as_ref()
            .ok_or(disk_errors::DRIVE_NOT_READY)?;

        let geometry = adapter.disk().geometry();
        let sectors = geometry.total_sectors() as u32;

        // Drive type:
        // 0x00 = not present
        // 0x01 = floppy without change-line support
        // 0x02 = floppy with change-line support
        // 0x03 = fixed disk (hard drive)
        let drive_type = if drive < 0x80 {
            0x02 // Floppy with change-line support
        } else {
            0x03 // Fixed disk
        };

        Ok((drive_type, sectors))
    }

    pub fn disk_detect_change(&mut self, drive: u8) -> Result<bool, u8> {
        let drive_state = self
            .get_drive_mut(drive)
            .ok_or(disk_errors::DRIVE_NOT_READY)?;

        if !drive_state.removable {
            // Fixed disks never change
            return Ok(false);
        }

        if !drive_state.has_disk() {
            return Err(disk_errors::DRIVE_NOT_READY);
        }

        // Return and clear the change flag
        let changed = drive_state.disk_changed;
        drive_state.disk_changed = false;
        Ok(changed)
    }
}
