use emu86_core::cpu::bios::{dos_errors, file_access, SeekMethod};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// File handle management for NativeBios
pub struct FileManager {
    open_files: HashMap<u16, File>,
    next_handle: u16,
    working_dir: PathBuf,
}

impl FileManager {
    pub fn new(working_dir: impl AsRef<Path>) -> Self {
        Self {
            open_files: HashMap::new(),
            next_handle: 3, // 0, 1, 2 are reserved for stdin/stdout/stderr
            working_dir: working_dir.as_ref().to_path_buf(),
        }
    }

    pub fn set_working_dir(&mut self, path: PathBuf) {
        self.working_dir = path;
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

    pub fn create(&mut self, filename: &str, _attributes: u8) -> Result<u16, u8> {
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

    pub fn open(&mut self, filename: &str, access_mode: u8) -> Result<u16, u8> {
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

    pub fn close(&mut self, handle: u16) -> Result<(), u8> {
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

    pub fn read(&mut self, handle: u16, max_bytes: u16) -> Result<Vec<u8>, u8> {
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

    pub fn write(&mut self, handle: u16, data: &[u8]) -> Result<u16, u8> {
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

    pub fn seek(&mut self, handle: u16, offset: i32, method: SeekMethod) -> Result<u32, u8> {
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

    pub fn contains_handle(&self, handle: u16) -> bool {
        self.open_files.contains_key(&handle)
    }
}
