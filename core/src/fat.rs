use crate::cpu::bios::{FindData, SeekMethod, disk_errors, dos_errors};
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
            // Read the sector
            let sector_data = self
                .disk
                .read_sector_lba(current_sector)
                .map_err(|e| Error::other(e.to_string()))?;

            // Calculate how many bytes to copy from this sector
            let sector_offset = if bytes_read == 0 { offset_in_sector } else { 0 };
            let bytes_in_sector = (SECTOR_SIZE - sector_offset).min(bytes_to_read - bytes_read);

            // Copy data from sector to buffer
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
            // Calculate how many bytes to write to this sector
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

            // Copy data from buffer to sector
            sector_data[sector_offset..sector_offset + bytes_in_sector]
                .copy_from_slice(&buf[buf_offset..buf_offset + bytes_in_sector]);

            // Write the sector
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

/// Metadata for an open file handle
struct FileHandle {
    path: String,
    position: u64,
    access_mode: u8,
}

/// Search state for directory iteration
struct SearchState {
    entries: Vec<FindData>,
    index: usize,
}

/// FAT filesystem wrapper around fatfs crate
/// Supports FAT12, FAT16, and FAT32 filesystems
pub struct FatFileSystem<D: DiskController> {
    adapter: DiskAdapter<D>,
    current_dir: String,
    open_files: HashMap<u16, FileHandle>,
    searches: HashMap<usize, SearchState>,
    next_handle: u16,
    next_search_id: usize,
}

impl<D: DiskController> FatFileSystem<D> {
    /// Create a new FAT filesystem from a disk controller
    /// Automatically detects FAT12, FAT16, or FAT32
    pub fn new(disk: D) -> Result<Self, String> {
        // Create adapter - filesystem validity will be checked on first use
        let adapter = DiskAdapter::new(disk);

        Ok(Self {
            adapter,
            current_dir: String::from("/"),
            open_files: HashMap::new(),
            searches: HashMap::new(),
            next_handle: 3, // 0-2 reserved for stdin/stdout/stderr
            next_search_id: 0,
        })
    }

    /// Set the next handle number (used by platform code to synchronize handle allocation)
    pub fn set_next_handle(&mut self, handle: u16) {
        self.next_handle = handle;
    }

    /// Helper: Reset adapter position to 0 (required by fatfs)
    fn reset_adapter_position(&mut self) {
        self.adapter.position = 0;
    }

    /// Convert DOS path (backslashes) to Unix path (forward slashes)
    fn dos_to_unix_path(dos_path: &str) -> String {
        dos_path.replace('\\', "/")
    }

    /// Convert IO error to DOS error code
    fn map_error(err: io::Error) -> u8 {
        match err.kind() {
            ErrorKind::NotFound => dos_errors::FILE_NOT_FOUND,
            ErrorKind::AlreadyExists => dos_errors::ACCESS_DENIED, // Closest match
            ErrorKind::PermissionDenied => dos_errors::ACCESS_DENIED,
            ErrorKind::InvalidInput => dos_errors::INVALID_FUNCTION,
            ErrorKind::WriteZero => dos_errors::ACCESS_DENIED, // Disk full
            _ => dos_errors::INVALID_FUNCTION,
        }
    }

    /// Resolve a path relative to current directory
    fn resolve_path(&self, path: &str) -> String {
        let unix_path = Self::dos_to_unix_path(path);

        if unix_path.starts_with('/') {
            // Absolute path
            unix_path
        } else {
            // Relative path - combine with current directory
            if self.current_dir == "/" {
                format!("/{}", unix_path)
            } else {
                format!("{}/{}", self.current_dir, unix_path)
            }
        }
    }

    // File operations using fatfs crate

    pub fn file_open(&mut self, filename: &str, access_mode: u8) -> Result<u16, u8> {
        let path = self.resolve_path(filename);
        log::debug!("FAT: Opening file '{}' (resolved to '{}')", filename, path);

        // Verify file exists by attempting to open it
        self.reset_adapter_position();
        let fs =
            fatfs::FileSystem::new(&mut self.adapter, fatfs::FsOptions::new()).map_err(|e| {
                log::error!("FAT: Failed to create filesystem: {:?}", e);
                Self::map_error(e)
            })?;
        let root_dir = fs.root_dir();

        // Try to open the file to verify it exists
        let _ = root_dir.open_file(&path).map_err(|e| {
            log::warn!("FAT: Failed to open file '{}': {:?}", path, e);
            Self::map_error(e)
        })?;

        log::debug!("FAT: File '{}' exists and can be opened", path);

        // Allocate handle
        let handle = self.next_handle;
        self.next_handle += 1;

        // Store file metadata
        self.open_files.insert(
            handle,
            FileHandle {
                path,
                position: 0,
                access_mode,
            },
        );

        Ok(handle)
    }

    pub fn file_create(&mut self, filename: &str, _attributes: u8) -> Result<u16, u8> {
        let path = self.resolve_path(filename);

        // Create the file using fatfs
        self.reset_adapter_position();
        let fs = fatfs::FileSystem::new(&mut self.adapter, fatfs::FsOptions::new())
            .map_err(Self::map_error)?;
        let root_dir = fs.root_dir();

        // Create file (truncates if exists)
        root_dir.create_file(&path).map_err(Self::map_error)?;

        // Allocate handle
        let handle = self.next_handle;
        self.next_handle += 1;

        // Store file metadata with write access
        self.open_files.insert(
            handle,
            FileHandle {
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
        let (path, position) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(dos_errors::INVALID_HANDLE)?;
            (file_handle.path.clone(), file_handle.position)
        };

        // Open filesystem and file
        self.reset_adapter_position();
        let fs = fatfs::FileSystem::new(&mut self.adapter, fatfs::FsOptions::new())
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

        // Update position
        let file_handle = self.open_files.get_mut(&handle).unwrap();
        file_handle.position += bytes_read as u64;

        Ok(buffer)
    }

    pub fn file_write(&mut self, handle: u16, data: &[u8]) -> Result<u16, u8> {
        // Get file info first, then release borrow
        let (path, position) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(dos_errors::INVALID_HANDLE)?;
            (file_handle.path.clone(), file_handle.position)
        };

        // Open filesystem and file
        self.reset_adapter_position();
        let fs = fatfs::FileSystem::new(&mut self.adapter, fatfs::FsOptions::new())
            .map_err(Self::map_error)?;
        let root_dir = fs.root_dir();
        let mut file = root_dir.open_file(&path).map_err(Self::map_error)?;

        // Seek to current position
        file.seek(SeekFrom::Start(position))
            .map_err(|_| dos_errors::INVALID_FUNCTION)?;

        // Write data
        let bytes_written = file.write(data).map_err(|_| dos_errors::INVALID_FUNCTION)?;

        // Update position
        let file_handle = self.open_files.get_mut(&handle).unwrap();
        file_handle.position += bytes_written as u64;

        Ok(bytes_written as u16)
    }

    pub fn file_seek(&mut self, handle: u16, offset: i32, method: SeekMethod) -> Result<u32, u8> {
        // Get file info first
        let (path, current_position) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(dos_errors::INVALID_HANDLE)?;
            (file_handle.path.clone(), file_handle.position)
        };

        // Calculate new position
        let new_position = match method {
            SeekMethod::FromStart => offset as i64,
            SeekMethod::FromCurrent => current_position as i64 + offset as i64,
            SeekMethod::FromEnd => {
                // Need to get file size by seeking to end
                self.reset_adapter_position();
                let fs = fatfs::FileSystem::new(&mut self.adapter, fatfs::FsOptions::new())
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
        let (path, position, access_mode) = {
            let file_handle = self
                .open_files
                .get(&handle)
                .ok_or(dos_errors::INVALID_HANDLE)?;
            (
                file_handle.path.clone(),
                file_handle.position,
                file_handle.access_mode,
            )
        };

        // Create new handle with same metadata
        let new_handle = self.next_handle;
        self.next_handle += 1;

        self.open_files.insert(
            new_handle,
            FileHandle {
                path,
                position,
                access_mode,
            },
        );

        Ok(new_handle)
    }

    // Directory operations

    pub fn dir_create(&mut self, dirname: &str) -> Result<(), u8> {
        let path = self.resolve_path(dirname);

        self.reset_adapter_position();
        let fs = fatfs::FileSystem::new(&mut self.adapter, fatfs::FsOptions::new())
            .map_err(Self::map_error)?;
        let root_dir = fs.root_dir();

        root_dir.create_dir(&path).map_err(Self::map_error)?;

        Ok(())
    }

    pub fn dir_remove(&mut self, dirname: &str) -> Result<(), u8> {
        let path = self.resolve_path(dirname);

        self.reset_adapter_position();
        let fs = fatfs::FileSystem::new(&mut self.adapter, fatfs::FsOptions::new())
            .map_err(Self::map_error)?;
        let root_dir = fs.root_dir();

        root_dir.remove(&path).map_err(Self::map_error)?;

        Ok(())
    }

    pub fn dir_change(&mut self, dirname: &str) -> Result<(), u8> {
        let path = self.resolve_path(dirname);

        // Verify directory exists
        self.reset_adapter_position();
        let fs = fatfs::FileSystem::new(&mut self.adapter, fatfs::FsOptions::new())
            .map_err(Self::map_error)?;
        let root_dir = fs.root_dir();

        // Try to open as directory
        let _dir = root_dir.open_dir(&path).map_err(Self::map_error)?;

        // Update current directory
        self.current_dir = path;

        Ok(())
    }

    pub fn dir_get_current(&self, _drive: u8) -> Result<String, u8> {
        // Convert Unix path back to DOS path
        let dos_path = self.current_dir.replace('/', "\\");
        Ok(dos_path)
    }

    pub fn find_first(&mut self, pattern: &str, attributes: u8) -> Result<(usize, FindData), u8> {
        let path = self.resolve_path(".");

        self.reset_adapter_position();
        let fs = fatfs::FileSystem::new(&mut self.adapter, fatfs::FsOptions::new())
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

        if entries.is_empty() {
            return Err(dos_errors::NO_MORE_FILES);
        }

        // Store search state
        let search_id = self.next_search_id;
        self.next_search_id += 1;

        let first_entry = entries[0].clone();

        self.searches
            .insert(search_id, SearchState { entries, index: 1 });

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

        // Handle simple patterns
        if pattern == "*.*" || pattern == "*" {
            return true;
        }

        // Simple wildcard matching (not full DOS pattern matching)
        // This is a simplified version - a full implementation would handle
        // 8.3 filenames and more complex patterns
        name.contains(&pattern.replace('*', ""))
    }

    /// Helper: Convert fatfs directory entry to FindData
    fn entry_to_find_data<T>(entry: &fatfs::DirEntry<'_, T>) -> Result<FindData, u8>
    where
        T: fatfs::ReadWriteSeek,
    {
        let name = entry.file_name();
        let is_dir = entry.is_dir();
        // Get file size - for directories it's 0, for files use metadata
        let size = if is_dir {
            0
        } else {
            // Open the file to get its size
            let mut file = entry.to_file();
            file.seek(SeekFrom::End(0)).unwrap_or(0) as u32
        };

        // Get modification time/date (fatfs provides this)
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

    /// Disk pass-through for INT 13h operations
    pub fn disk_read_sectors(
        &self,
        _drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, u8> {
        let mut result = Vec::new();

        for i in 0..count {
            let sector_data = self
                .adapter
                .disk()
                .read_sector_chs(cylinder as u16, head as u16, (sector + i) as u16)
                .map_err(|_| disk_errors::SECTOR_NOT_FOUND)?;
            result.extend_from_slice(&sector_data);
        }

        Ok(result)
    }

    pub fn disk_write_sectors(
        &mut self,
        _drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
        data: &[u8],
    ) -> Result<u8, u8> {
        if self.adapter.disk().is_read_only() {
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

            self.adapter
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

    /// Check if a handle is valid (opened file)
    pub fn contains_handle(&self, handle: u16) -> bool {
        self.open_files.contains_key(&handle)
    }

    pub fn disk_reset(&mut self, _drive: u8) -> bool {
        true // Always succeeds in our implementation
    }

    pub fn disk_get_params(&self, _drive: u8) -> Result<crate::cpu::bios::DriveParams, u8> {
        let geometry = self.adapter.disk().geometry();

        Ok(crate::cpu::bios::DriveParams {
            max_cylinder: (geometry.cylinders - 1).min(255) as u8,
            max_head: (geometry.heads - 1).min(255) as u8,
            max_sector: geometry.sectors_per_track.min(255) as u8,
            drive_count: 1,
        })
    }

    pub fn disk_get_type(&self, _drive: u8) -> Result<(u8, u32), u8> {
        let geometry = self.adapter.disk().geometry();
        let sectors = geometry.total_sectors() as u32;

        // Drive type: 0x02 for floppy with change-line support
        Ok((0x02, sectors))
    }

    pub fn disk_detect_change(&mut self, _drive: u8) -> Result<bool, u8> {
        // In an emulator, the disk image doesn't physically change
        // Always return false (disk not changed)
        Ok(false)
    }
}
