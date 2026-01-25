use emu86_core::cpu::bios::{
    DriveParams, FindData, SeekMethod, disk_errors, dos_errors, file_access, file_attributes,
};
/// Standard I/O implementation of Bios for native platform
use emu86_core::{Bios, DiskController, SECTOR_SIZE};
use std::collections::HashMap;
use std::fs::{DirBuilder, File, OpenOptions, ReadDir};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// State for an active directory search
struct SearchState {
    /// Directory reader iterator
    entries: ReadDir,
    /// File pattern to match (supports * and ? wildcards)
    pattern: String,
    /// Attributes to match
    attributes: u8,
}

pub struct StdioBios<D: DiskController> {
    disk: D,
    open_files: HashMap<u16, File>,
    next_handle: u16,
    working_dir: PathBuf,
    /// Active directory searches (indexed by search_id)
    searches: HashMap<usize, SearchState>,
    /// Next search ID to allocate
    next_search_id: usize,
}

impl<D: DiskController> StdioBios<D> {
    pub fn new(disk: D, working_dir: impl AsRef<Path>) -> Self {
        Self {
            disk,
            open_files: HashMap::new(),
            next_handle: 3, // 0, 1, 2 are reserved for stdin/stdout/stderr
            working_dir: working_dir.as_ref().to_path_buf(),
            searches: HashMap::new(),
            next_search_id: 0,
        }
    }

    /// Allocate a new file handle
    fn allocate_handle(&mut self) -> Option<u16> {
        if self.open_files.len() >= 252 {
            // Limit to 252 user files (handles 3-254)
            return None;
        }
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        if self.next_handle < 3 {
            self.next_handle = 3; // Wrap around but skip reserved handles
        }
        Some(handle)
    }

    /// Resolve a filename relative to the working directory
    fn resolve_path(&self, filename: &str) -> PathBuf {
        let path = Path::new(filename);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_dir.join(path)
        }
    }

    /// Convert DOS pattern (with * and ?) to a simple matcher
    /// Returns true if filename matches pattern
    fn matches_pattern(filename: &str, pattern: &str) -> bool {
        // Simple wildcard matching (case-insensitive)
        // * matches any sequence of characters
        // ? matches any single character

        let filename_upper = filename.to_ascii_uppercase();
        let pattern_upper = pattern.to_ascii_uppercase();

        Self::matches_pattern_impl(&filename_upper, &pattern_upper)
    }

    fn matches_pattern_impl(filename: &str, pattern: &str) -> bool {
        let mut pattern_chars = pattern.chars().peekable();
        let mut filename_chars = filename.chars().peekable();

        while let Some(&p) = pattern_chars.peek() {
            match p {
                '*' => {
                    pattern_chars.next();
                    // If * is at the end, match everything
                    if pattern_chars.peek().is_none() {
                        return true;
                    }
                    // Try to match the rest of the pattern with any suffix
                    loop {
                        let remaining_filename: String = filename_chars.clone().collect();
                        let remaining_pattern: String = pattern_chars.clone().collect();

                        if Self::matches_pattern_impl(&remaining_filename, &remaining_pattern) {
                            return true;
                        }

                        if filename_chars.next().is_none() {
                            return false;
                        }
                    }
                }
                '?' => {
                    pattern_chars.next();
                    if filename_chars.next().is_none() {
                        return false;
                    }
                }
                c => {
                    pattern_chars.next();
                    match filename_chars.next() {
                        Some(fc) if fc == c => continue,
                        _ => return false,
                    }
                }
            }
        }

        // Pattern exhausted, filename should also be exhausted
        filename_chars.peek().is_none()
    }

    /// Convert file metadata to DOS format
    fn file_to_find_data(entry: &std::fs::DirEntry) -> io::Result<FindData> {
        let metadata = entry.metadata()?;
        let filename = entry.file_name().to_string_lossy().to_string();

        // Get file attributes
        let mut attributes = 0u8;
        if metadata.is_dir() {
            attributes |= file_attributes::DIRECTORY;
        }
        if metadata.permissions().readonly() {
            attributes |= file_attributes::READ_ONLY;
        }

        // Get file size
        let size = if metadata.is_file() {
            metadata.len() as u32
        } else {
            0
        };

        // Get modified time and convert to DOS format
        let (time, date) = if let Ok(modified) = metadata.modified() {
            Self::system_time_to_dos_datetime(modified)
        } else {
            (0, 0)
        };

        Ok(FindData {
            attributes,
            time,
            date,
            size,
            filename,
        })
    }

    /// Convert SystemTime to DOS date/time format
    fn system_time_to_dos_datetime(time: std::time::SystemTime) -> (u16, u16) {
        use std::time::UNIX_EPOCH;

        // Get seconds since Unix epoch
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
        let secs = duration.as_secs();

        // Convert to DOS format (simplified)
        // DOS time: bits 0-4: seconds/2, 5-10: minutes, 11-15: hours
        // DOS date: bits 0-4: day, 5-8: month, 9-15: year-1980

        // Simple conversion (not handling timezones properly)
        let days_since_epoch = secs / 86400;
        let time_of_day = secs % 86400;

        let hours = (time_of_day / 3600) as u16;
        let minutes = ((time_of_day % 3600) / 60) as u16;
        let seconds = ((time_of_day % 60) / 2) as u16;

        // Approximate date calculation
        let year = 1970 + (days_since_epoch / 365) as u16;
        let month = 1u16; // Simplified
        let day = 1u16; // Simplified

        let dos_time = (hours << 11) | (minutes << 5) | seconds;
        let dos_date = ((year.saturating_sub(1980)) << 9) | (month << 5) | day;

        (dos_time, dos_date)
    }
}

impl<D: DiskController> Bios for StdioBios<D> {
    fn read_char(&mut self) -> Option<u8> {
        let mut buffer = [0u8; 1];
        match io::stdin().read_exact(&mut buffer) {
            Ok(_) => Some(buffer[0]),
            Err(_) => None,
        }
    }

    fn write_char(&mut self, ch: u8) {
        print!("{}", ch as char);
        let _ = io::stdout().flush();
    }

    fn write_str(&mut self, s: &str) {
        print!("{}", s);
        let _ = io::stdout().flush();
    }

    fn disk_reset(&mut self, _drive: u8) -> bool {
        // Always succeed for reset
        true
    }

    fn disk_read_sectors(
        &mut self,
        _drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, u8> {
        let mut result = Vec::with_capacity(count as usize * SECTOR_SIZE);

        for i in 0..count {
            // Calculate CHS for each sector
            let current_sector = sector + i;

            match self
                .disk
                .read_sector_chs(cylinder as u16, head as u16, current_sector as u16)
            {
                Ok(sector_data) => {
                    result.extend_from_slice(&sector_data);
                }
                Err(_) => {
                    return Err(disk_errors::SECTOR_NOT_FOUND);
                }
            }
        }

        Ok(result)
    }

    fn disk_write_sectors(
        &mut self,
        _drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
        data: &[u8],
    ) -> Result<u8, u8> {
        if self.disk.is_read_only() {
            return Err(disk_errors::WRITE_PROTECTED);
        }

        let mut sectors_written = 0;

        for i in 0..count {
            let offset = i as usize * SECTOR_SIZE;
            if offset + SECTOR_SIZE > data.len() {
                break;
            }

            let current_sector = sector + i;
            let mut sector_data = [0u8; SECTOR_SIZE];
            sector_data.copy_from_slice(&data[offset..offset + SECTOR_SIZE]);

            match self.disk.write_sector_chs(
                cylinder as u16,
                head as u16,
                current_sector as u16,
                &sector_data,
            ) {
                Ok(_) => {
                    sectors_written += 1;
                }
                Err(_) => {
                    if sectors_written == 0 {
                        return Err(disk_errors::SECTOR_NOT_FOUND);
                    } else {
                        return Ok(sectors_written);
                    }
                }
            }
        }

        Ok(sectors_written)
    }

    fn disk_get_params(&self, _drive: u8) -> Result<DriveParams, u8> {
        let geom = self.disk.geometry();
        Ok(DriveParams {
            max_cylinder: (geom.cylinders - 1).min(255) as u8,
            max_head: (geom.heads - 1).min(255) as u8,
            max_sector: geom.sectors_per_track.min(63) as u8,
            drive_count: 1,
        })
    }

    fn file_create(&mut self, filename: &str, _attributes: u8) -> Result<u16, u8> {
        let handle = self
            .allocate_handle()
            .ok_or(dos_errors::TOO_MANY_OPEN_FILES)?;

        let path = self.resolve_path(filename);

        match OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
        {
            Ok(file) => {
                self.open_files.insert(handle, file);
                Ok(handle)
            }
            Err(e) => {
                let error_code = match e.kind() {
                    io::ErrorKind::PermissionDenied => dos_errors::ACCESS_DENIED,
                    io::ErrorKind::NotFound => dos_errors::PATH_NOT_FOUND,
                    _ => dos_errors::ACCESS_DENIED,
                };
                Err(error_code)
            }
        }
    }

    fn file_open(&mut self, filename: &str, access_mode: u8) -> Result<u16, u8> {
        let handle = self
            .allocate_handle()
            .ok_or(dos_errors::TOO_MANY_OPEN_FILES)?;

        let path = self.resolve_path(filename);

        let mut options = OpenOptions::new();
        match access_mode {
            file_access::READ_ONLY => {
                options.read(true);
            }
            file_access::WRITE_ONLY => {
                options.write(true);
            }
            file_access::READ_WRITE => {
                options.read(true).write(true);
            }
            _ => return Err(dos_errors::INVALID_ACCESS_CODE),
        }

        match options.open(&path) {
            Ok(file) => {
                self.open_files.insert(handle, file);
                Ok(handle)
            }
            Err(e) => {
                let error_code = match e.kind() {
                    io::ErrorKind::NotFound => dos_errors::FILE_NOT_FOUND,
                    io::ErrorKind::PermissionDenied => dos_errors::ACCESS_DENIED,
                    _ => dos_errors::FILE_NOT_FOUND,
                };
                Err(error_code)
            }
        }
    }

    fn file_close(&mut self, handle: u16) -> Result<(), u8> {
        // Don't allow closing standard handles
        if handle < 3 {
            return Err(dos_errors::INVALID_HANDLE);
        }

        if self.open_files.remove(&handle).is_some() {
            Ok(())
        } else {
            Err(dos_errors::INVALID_HANDLE)
        }
    }

    fn file_read(&mut self, handle: u16, max_bytes: u16) -> Result<Vec<u8>, u8> {
        // Handle stdin separately
        if handle == 0 {
            let mut buffer = vec![0u8; max_bytes as usize];
            match io::stdin().read(&mut buffer) {
                Ok(n) => {
                    buffer.truncate(n);
                    Ok(buffer)
                }
                Err(_) => Err(dos_errors::ACCESS_DENIED),
            }
        } else if let Some(file) = self.open_files.get_mut(&handle) {
            let mut buffer = vec![0u8; max_bytes as usize];
            match file.read(&mut buffer) {
                Ok(n) => {
                    buffer.truncate(n);
                    Ok(buffer)
                }
                Err(_) => Err(dos_errors::ACCESS_DENIED),
            }
        } else {
            Err(dos_errors::INVALID_HANDLE)
        }
    }

    fn file_write(&mut self, handle: u16, data: &[u8]) -> Result<u16, u8> {
        // Handle stdout/stderr separately
        if handle == 1 {
            match io::stdout().write(data) {
                Ok(n) => {
                    let _ = io::stdout().flush();
                    Ok(n as u16)
                }
                Err(_) => Err(dos_errors::ACCESS_DENIED),
            }
        } else if handle == 2 {
            match io::stderr().write(data) {
                Ok(n) => {
                    let _ = io::stderr().flush();
                    Ok(n as u16)
                }
                Err(_) => Err(dos_errors::ACCESS_DENIED),
            }
        } else if let Some(file) = self.open_files.get_mut(&handle) {
            match file.write(data) {
                Ok(n) => Ok(n as u16),
                Err(_) => Err(dos_errors::ACCESS_DENIED),
            }
        } else {
            Err(dos_errors::INVALID_HANDLE)
        }
    }

    fn file_seek(&mut self, handle: u16, offset: i32, method: SeekMethod) -> Result<u32, u8> {
        // Standard handles don't support seeking
        if handle < 3 {
            return Err(dos_errors::INVALID_HANDLE);
        }

        if let Some(file) = self.open_files.get_mut(&handle) {
            let seek_from = match method {
                SeekMethod::FromStart => SeekFrom::Start(offset.max(0) as u64),
                SeekMethod::FromCurrent => SeekFrom::Current(offset as i64),
                SeekMethod::FromEnd => SeekFrom::End(offset as i64),
            };

            match file.seek(seek_from) {
                Ok(pos) => Ok(pos as u32),
                Err(_) => Err(dos_errors::ACCESS_DENIED),
            }
        } else {
            Err(dos_errors::INVALID_HANDLE)
        }
    }

    fn dir_create(&mut self, dirname: &str) -> Result<(), u8> {
        let path = self.resolve_path(dirname);

        match DirBuilder::new().create(&path) {
            Ok(_) => Ok(()),
            Err(e) => {
                let error_code = match e.kind() {
                    io::ErrorKind::PermissionDenied => dos_errors::ACCESS_DENIED,
                    io::ErrorKind::AlreadyExists => dos_errors::ACCESS_DENIED,
                    io::ErrorKind::NotFound => dos_errors::PATH_NOT_FOUND,
                    _ => dos_errors::ACCESS_DENIED,
                };
                Err(error_code)
            }
        }
    }

    fn dir_remove(&mut self, dirname: &str) -> Result<(), u8> {
        let path = self.resolve_path(dirname);

        // Check if it's the current directory
        if path == self.working_dir {
            return Err(dos_errors::ATTEMPT_TO_REMOVE_CURRENT_DIR);
        }

        match std::fs::remove_dir(&path) {
            Ok(_) => Ok(()),
            Err(e) => {
                let error_code = match e.kind() {
                    io::ErrorKind::PermissionDenied => dos_errors::ACCESS_DENIED,
                    io::ErrorKind::NotFound => dos_errors::PATH_NOT_FOUND,
                    _ => dos_errors::ACCESS_DENIED,
                };
                Err(error_code)
            }
        }
    }

    fn dir_change(&mut self, dirname: &str) -> Result<(), u8> {
        let path = self.resolve_path(dirname);

        // Verify the directory exists
        if !path.exists() {
            return Err(dos_errors::PATH_NOT_FOUND);
        }

        if !path.is_dir() {
            return Err(dos_errors::PATH_NOT_FOUND);
        }

        // Update working directory
        self.working_dir = path
            .canonicalize()
            .map_err(|_| dos_errors::PATH_NOT_FOUND)?;

        Ok(())
    }

    fn dir_get_current(&self, _drive: u8) -> Result<String, u8> {
        // Convert absolute path to a relative path string (without drive letter)
        // For Unix-like systems, we'll just return the path as-is
        // For a real DOS implementation, we'd need to strip the drive letter

        let path_str = self.working_dir.to_string_lossy();

        // Remove leading slash for DOS compatibility
        let path_str = path_str.strip_prefix('/').unwrap_or(&path_str);

        Ok(path_str.to_string())
    }

    fn find_first(&mut self, pattern: &str, attributes: u8) -> Result<(usize, FindData), u8> {
        let path = self.resolve_path(pattern);

        // Separate directory from pattern
        let (dir_path, file_pattern) = if let Some(parent) = path.parent() {
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("*");
            (parent.to_path_buf(), filename.to_string())
        } else {
            (self.working_dir.clone(), pattern.to_string())
        };

        // Open directory
        let entries = match std::fs::read_dir(&dir_path) {
            Ok(entries) => entries,
            Err(e) => {
                let error_code = match e.kind() {
                    io::ErrorKind::NotFound => dos_errors::PATH_NOT_FOUND,
                    io::ErrorKind::PermissionDenied => dos_errors::ACCESS_DENIED,
                    _ => dos_errors::PATH_NOT_FOUND,
                };
                return Err(error_code);
            }
        };

        // Allocate search ID
        let search_id = self.next_search_id;
        self.next_search_id = self.next_search_id.wrapping_add(1);

        // Create search state
        let mut search_state = SearchState {
            entries,
            pattern: file_pattern,
            attributes,
        };

        // Find first matching entry
        let find_data = Self::find_next_matching(&mut search_state)?;

        // Store search state
        self.searches.insert(search_id, search_state);

        Ok((search_id, find_data))
    }

    fn find_next(&mut self, search_id: usize) -> Result<FindData, u8> {
        let search_state = self
            .searches
            .get_mut(&search_id)
            .ok_or(dos_errors::NO_MORE_FILES)?;

        Self::find_next_matching(search_state)
    }

    fn get_current_drive(&self) -> u8 {
        // For Unix-like systems, we don't have drive letters
        // Always return drive A (0)
        0
    }
}

impl<D: DiskController> StdioBios<D> {
    /// Find the next matching file in a search
    fn find_next_matching(search_state: &mut SearchState) -> Result<FindData, u8> {
        loop {
            let entry = search_state
                .entries
                .next()
                .ok_or(dos_errors::NO_MORE_FILES)?
                .map_err(|_| dos_errors::NO_MORE_FILES)?;

            // Get file info
            let find_data =
                Self::file_to_find_data(&entry).map_err(|_| dos_errors::NO_MORE_FILES)?;

            // Check if filename matches pattern
            if !Self::matches_pattern(&find_data.filename, &search_state.pattern) {
                continue;
            }

            // Check if attributes match
            // If searching for directories, only return directories
            // If searching for files, return files (and optionally hidden/system files based on attributes)
            if (search_state.attributes & file_attributes::DIRECTORY) != 0 {
                // Searching for directories
                if (find_data.attributes & file_attributes::DIRECTORY) == 0 {
                    continue;
                }
            }

            return Ok(find_data);
        }
    }
}
