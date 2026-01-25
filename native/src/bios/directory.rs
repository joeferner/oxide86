use emu86_core::cpu::bios::{dos_errors, file_attributes, FindData};
use std::collections::HashMap;
use std::fs::{DirBuilder, ReadDir};
use std::io;
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

/// Directory search management for NativeBios
pub struct DirectoryManager {
    working_dir: PathBuf,
    /// Active directory searches (indexed by search_id)
    searches: HashMap<usize, SearchState>,
    /// Next search ID to allocate
    next_search_id: usize,
}

impl DirectoryManager {
    pub fn new(working_dir: impl AsRef<Path>) -> Self {
        Self {
            working_dir: working_dir.as_ref().to_path_buf(),
            searches: HashMap::new(),
            next_search_id: 0,
        }
    }

    pub fn working_dir(&self) -> &Path {
        &self.working_dir
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

    pub fn create(&mut self, dirname: &str) -> Result<(), u8> {
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

    pub fn remove(&mut self, dirname: &str) -> Result<(), u8> {
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

    pub fn change(&mut self, dirname: &str) -> Result<(), u8> {
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

    pub fn get_current(&self, _drive: u8) -> Result<String, u8> {
        // Convert absolute path to a relative path string (without drive letter)
        // For Unix-like systems, we'll just return the path as-is
        // For a real DOS implementation, we'd need to strip the drive letter

        let path_str = self.working_dir.to_string_lossy();

        // Remove leading slash for DOS compatibility
        let path_str = path_str.strip_prefix('/').unwrap_or(&path_str);

        Ok(path_str.to_string())
    }

    pub fn find_first(&mut self, pattern: &str, attributes: u8) -> Result<(usize, FindData), u8> {
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
        let find_data = find_next_matching(&mut search_state)?;

        // Store search state
        self.searches.insert(search_id, search_state);

        Ok((search_id, find_data))
    }

    pub fn find_next(&mut self, search_id: usize) -> Result<FindData, u8> {
        let search_state = self
            .searches
            .get_mut(&search_id)
            .ok_or(dos_errors::NO_MORE_FILES)?;

        find_next_matching(search_state)
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

    matches_pattern_impl(&filename_upper, &pattern_upper)
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

                    if matches_pattern_impl(&remaining_filename, &remaining_pattern) {
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
        system_time_to_dos_datetime(modified)
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

/// Find the next matching file in a search
fn find_next_matching(search_state: &mut SearchState) -> Result<FindData, u8> {
    loop {
        let entry = search_state
            .entries
            .next()
            .ok_or(dos_errors::NO_MORE_FILES)?
            .map_err(|_| dos_errors::NO_MORE_FILES)?;

        // Get file info
        let find_data = file_to_find_data(&entry).map_err(|_| dos_errors::NO_MORE_FILES)?;

        // Check if filename matches pattern
        if !matches_pattern(&find_data.filename, &search_state.pattern) {
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
