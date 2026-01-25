/// Standard I/O implementation of Bios for native platform
use emu86_core::{Bios, DiskController, SECTOR_SIZE};
use emu86_core::cpu::bios::{DriveParams, SeekMethod, disk_errors, dos_errors, file_access};
use std::collections::HashMap;
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

pub struct StdioBios<D: DiskController> {
    disk: D,
    open_files: HashMap<u16, File>,
    next_handle: u16,
    working_dir: PathBuf,
}

impl<D: DiskController> StdioBios<D> {
    pub fn new(disk: D, working_dir: impl AsRef<Path>) -> Self {
        Self {
            disk,
            open_files: HashMap::new(),
            next_handle: 3, // 0, 1, 2 are reserved for stdin/stdout/stderr
            working_dir: working_dir.as_ref().to_path_buf(),
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

            match self.disk.read_sector_chs(cylinder as u16, head as u16, current_sector as u16) {
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

            match self.disk.write_sector_chs(cylinder as u16, head as u16, current_sector as u16, &sector_data) {
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
        let handle = self.allocate_handle()
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
        let handle = self.allocate_handle()
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
}
