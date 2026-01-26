mod console;
mod memory_allocator;
mod peripheral;
mod time;

use emu86_core::cpu::bios::{
    DriveParams, ExecParams, FindData, KeyPress, PrinterStatus, RtcDate, RtcTime, SeekMethod,
    SerialParams, SerialStatus, dos_errors, file_access,
};
use emu86_core::{Bios, DiskController, FatFileSystem};
use std::collections::HashMap;
use std::io::{self, Read, Write};

use crate::bios::memory_allocator::MemoryAllocator;

/// DOS device types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DosDevice {
    Null,    // NUL device
    Console, // CON device
}

/// Native platform implementation of BIOS
pub struct NativeBios<D: DiskController> {
    fat: FatFileSystem<D>,
    memory_allocator: MemoryAllocator,
    device_handles: HashMap<u16, DosDevice>,
    /// Next file/device handle to allocate. Shared between device handles and file handles.
    next_handle: u16,
}

impl<D: DiskController> NativeBios<D> {
    pub fn new(disk: D) -> Result<Self, String> {
        let mut fat = FatFileSystem::new(disk)?;
        // Sync the FAT filesystem's next_handle with our next_handle
        fat.set_next_handle(3);
        Ok(Self {
            fat,
            memory_allocator: MemoryAllocator::new(),
            device_handles: HashMap::new(),
            next_handle: 3, // 0, 1, 2 are reserved for stdin/stdout/stderr
        })
    }

    /// Check if a filename is a DOS device name
    fn is_dos_device(filename: &str) -> Option<DosDevice> {
        // DOS device names are case-insensitive and may have extensions
        let name = filename.to_uppercase();
        let base_name = name.split('.').next().unwrap_or(&name);

        match base_name {
            "NUL" => Some(DosDevice::Null),
            "CON" => Some(DosDevice::Console),
            "AUX" | "COM1" | "COM2" | "COM3" | "COM4" => Some(DosDevice::Null), // Serial ports - stub as null
            "PRN" | "LPT1" | "LPT2" | "LPT3" => Some(DosDevice::Null), // Printer ports - stub as null
            _ => None,
        }
    }

    /// Allocate a new file/device handle
    /// This is shared between device handles and file handles to avoid collisions
    fn allocate_handle(&mut self) -> Option<u16> {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        if self.next_handle < 3 {
            self.next_handle = 3; // Wrap around but skip reserved handles
        }
        // Sync the handle counter with the FAT filesystem
        self.fat.set_next_handle(self.next_handle);
        Some(handle)
    }
}

impl<D: DiskController> Bios for NativeBios<D> {
    // Console I/O
    fn read_char(&mut self) -> Option<u8> {
        console::read_char()
    }

    fn write_char(&mut self, ch: u8) {
        console::write_char(ch);
    }

    fn write_str(&mut self, s: &str) {
        console::write_str(s);
    }

    fn read_key(&mut self) -> Option<KeyPress> {
        console::read_key()
    }

    // Disk operations - delegate to FatFileSystem which owns the disk
    fn disk_reset(&mut self, drive: u8) -> bool {
        self.fat.disk_reset(drive)
    }

    fn disk_read_sectors(
        &mut self,
        drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, u8> {
        self.fat
            .disk_read_sectors(drive, cylinder, head, sector, count)
    }

    fn disk_write_sectors(
        &mut self,
        drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
        data: &[u8],
    ) -> Result<u8, u8> {
        self.fat
            .disk_write_sectors(drive, cylinder, head, sector, count, data)
    }

    fn disk_get_params(&self, drive: u8) -> Result<DriveParams, u8> {
        self.fat.disk_get_params(drive)
    }

    fn disk_get_type(&self, drive: u8) -> Result<(u8, u32), u8> {
        self.fat.disk_get_type(drive)
    }

    // File operations
    fn file_create(&mut self, filename: &str, attributes: u8) -> Result<u16, u8> {
        // Check if it's a DOS device
        if let Some(device) = Self::is_dos_device(filename) {
            let handle = self
                .allocate_handle()
                .ok_or(dos_errors::TOO_MANY_OPEN_FILES)?;
            self.device_handles.insert(handle, device);
            return Ok(handle);
        }

        // Delegate to FAT filesystem
        self.fat.file_create(filename, attributes)
    }

    fn file_open(&mut self, filename: &str, access_mode: u8) -> Result<u16, u8> {
        // Check if it's a DOS device
        if let Some(device) = Self::is_dos_device(filename) {
            let handle = self
                .allocate_handle()
                .ok_or(dos_errors::TOO_MANY_OPEN_FILES)?;
            self.device_handles.insert(handle, device);
            return Ok(handle);
        }

        // Delegate to FAT filesystem
        self.fat.file_open(filename, access_mode)
    }

    fn file_close(&mut self, handle: u16) -> Result<(), u8> {
        // Don't allow closing standard handles
        if handle < 3 {
            return Err(dos_errors::INVALID_HANDLE);
        }

        // Try removing from device handles first
        if self.device_handles.remove(&handle).is_some() {
            return Ok(());
        }

        // Delegate to FAT filesystem
        self.fat.file_close(handle)
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
        } else if let Some(device) = self.device_handles.get(&handle) {
            // Handle DOS devices
            match device {
                DosDevice::Null => {
                    // NUL always returns EOF (0 bytes)
                    Ok(Vec::new())
                }
                DosDevice::Console => {
                    // CON reads from stdin
                    let mut buffer = vec![0u8; max_bytes as usize];
                    match io::stdin().read(&mut buffer) {
                        Ok(n) => {
                            buffer.truncate(n);
                            Ok(buffer)
                        }
                        Err(_) => Err(dos_errors::ACCESS_DENIED),
                    }
                }
            }
        } else {
            // Delegate to FAT filesystem
            self.fat.file_read(handle, max_bytes)
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
        } else if let Some(device) = self.device_handles.get(&handle) {
            // Handle DOS devices
            match device {
                DosDevice::Null => {
                    // NUL discards all data but reports success
                    Ok(data.len() as u16)
                }
                DosDevice::Console => {
                    // CON writes to stdout
                    match io::stdout().write(data) {
                        Ok(n) => {
                            let _ = io::stdout().flush();
                            Ok(n as u16)
                        }
                        Err(_) => Err(dos_errors::ACCESS_DENIED),
                    }
                }
            }
        } else {
            // Delegate to FAT filesystem
            self.fat.file_write(handle, data)
        }
    }

    fn file_seek(&mut self, handle: u16, offset: i32, method: SeekMethod) -> Result<u32, u8> {
        // Standard handles and device handles don't support seeking
        if handle < 3 || self.device_handles.contains_key(&handle) {
            return Err(dos_errors::INVALID_HANDLE);
        }

        // Delegate to FAT filesystem
        self.fat.file_seek(handle, offset, method)
    }

    fn file_duplicate(&mut self, handle: u16) -> Result<u16, u8> {
        // Standard handles (0, 1, 2) can be duplicated
        if handle < 3 {
            let new_handle = self
                .allocate_handle()
                .ok_or(dos_errors::TOO_MANY_OPEN_FILES)?;
            // We don't actually store anything for standard handles
            return Ok(new_handle);
        }

        // Check if it's a device handle
        if let Some(device) = self.device_handles.get(&handle).copied() {
            let new_handle = self
                .allocate_handle()
                .ok_or(dos_errors::TOO_MANY_OPEN_FILES)?;
            self.device_handles.insert(new_handle, device);
            return Ok(new_handle);
        }

        // Delegate to FAT filesystem
        self.fat.file_duplicate(handle)
    }

    // Directory operations
    fn dir_create(&mut self, dirname: &str) -> Result<(), u8> {
        self.fat.dir_create(dirname)
    }

    fn dir_remove(&mut self, dirname: &str) -> Result<(), u8> {
        self.fat.dir_remove(dirname)
    }

    fn dir_change(&mut self, dirname: &str) -> Result<(), u8> {
        self.fat.dir_change(dirname)
    }

    fn dir_get_current(&self, drive: u8) -> Result<String, u8> {
        self.fat.dir_get_current(drive)
    }

    fn find_first(&mut self, pattern: &str, attributes: u8) -> Result<(usize, FindData), u8> {
        self.fat.find_first(pattern, attributes)
    }

    fn find_next(&mut self, search_id: usize) -> Result<FindData, u8> {
        self.fat.find_next(search_id)
    }

    // Drive management
    fn get_current_drive(&self) -> u8 {
        // For Unix-like systems, we don't have drive letters
        // Always return drive A (0)
        0
    }

    fn set_default_drive(&mut self, _drive: u8) -> u8 {
        // For Unix-like systems, we don't have drive letters
        // Always return 1 logical drive (A:)
        1
    }

    // Memory management
    fn memory_allocate(&mut self, paragraphs: u16) -> Result<u16, (u8, u16)> {
        self.memory_allocator.allocate(paragraphs)
    }

    fn memory_free(&mut self, segment: u16) -> Result<(), u8> {
        self.memory_allocator.free(segment)
    }

    fn memory_resize(&mut self, segment: u16, paragraphs: u16) -> Result<(), (u8, u16)> {
        self.memory_allocator.resize(segment, paragraphs)
    }

    // PSP management
    fn get_psp(&self) -> u16 {
        // Default PSP segment for simple programs
        0x0100
    }

    fn set_psp(&mut self, _segment: u16) {
        // PSP tracking is not implemented in this simple BIOS
    }

    // IOCTL
    fn ioctl_get_device_info(&self, handle: u16) -> Result<u16, u8> {
        // Return device information word
        // Bit 7 = 1 for character device, 0 for disk file
        // Bit 6 = 0 for EOF on input (for files)
        // Bit 5 = 0 for binary mode (raw), 1 for cooked mode
        // Bit 0 = 1 for console input (stdin)
        // Bit 1 = 1 for console output (stdout)
        match handle {
            0 => Ok(0x80D1), // STDIN: device (bit 7), console input (bit 0)
            1 => Ok(0x80D2), // STDOUT: device (bit 7), console output (bit 1)
            2 => Ok(0x80D2), // STDERR: device (bit 7), console output (bit 1)
            _ => {
                // Check if it's a DOS device handle
                if let Some(device) = self.device_handles.get(&handle) {
                    // Return device info based on device type
                    match device {
                        DosDevice::Null => Ok(0x8004),    // NUL device (bit 7), special device
                        DosDevice::Console => Ok(0x80D3), // CON device (bit 7), console I/O
                    }
                } else if self.fat.contains_handle(handle) {
                    // It's a regular file (bit 7 = 0)
                    Ok(0x0000)
                } else {
                    Err(dos_errors::INVALID_HANDLE)
                }
            }
        }
    }

    fn ioctl_set_device_info(&mut self, handle: u16, _info: u16) -> Result<(), u8> {
        // Allow setting device info for standard handles and open files
        match handle {
            0..=2 => {
                // Standard handles - allow setting but ignore
                Ok(())
            }
            _ => {
                // Check if it's a valid file handle
                if self.fat.contains_handle(handle) || self.device_handles.contains_key(&handle) {
                    // Allow setting but ignore for files and devices
                    Ok(())
                } else {
                    Err(dos_errors::INVALID_HANDLE)
                }
            }
        }
    }

    fn exec_load_program(&mut self, params: &ExecParams) -> Result<Vec<u8>, u8> {
        // Open the program file
        let handle = self.file_open(&params.filename, file_access::READ_ONLY)?;

        // Read the entire file
        let mut program_data = Vec::new();
        loop {
            match self.file_read(handle, 4096) {
                Ok(data) => {
                    if data.is_empty() {
                        break;
                    }
                    program_data.extend(data);
                }
                Err(e) => {
                    let _ = self.file_close(handle);
                    return Err(e);
                }
            }
        }

        // Close the file
        let _ = self.file_close(handle);

        if program_data.is_empty() {
            return Err(dos_errors::FILE_NOT_FOUND);
        }

        Ok(program_data)
    }

    // Serial port (stub implementations)
    fn serial_init(&mut self, port: u8, params: SerialParams) -> SerialStatus {
        peripheral::serial_init(port, params)
    }

    fn serial_write(&mut self, port: u8, ch: u8) -> u8 {
        peripheral::serial_write(port, ch)
    }

    fn serial_read(&mut self, port: u8) -> Result<(u8, u8), u8> {
        peripheral::serial_read(port)
    }

    fn serial_status(&self, port: u8) -> SerialStatus {
        peripheral::serial_status(port)
    }

    // Printer (stub implementations)
    fn printer_init(&mut self, printer: u8) -> PrinterStatus {
        peripheral::printer_init(printer)
    }

    fn printer_write(&mut self, printer: u8, ch: u8) -> PrinterStatus {
        peripheral::printer_write(printer, ch)
    }

    fn printer_status(&self, printer: u8) -> PrinterStatus {
        peripheral::printer_status(printer)
    }

    // Time and RTC
    fn get_system_ticks(&self) -> u32 {
        time::get_system_ticks()
    }

    fn get_rtc_time(&self) -> Option<RtcTime> {
        time::get_rtc_time()
    }

    fn get_rtc_date(&self) -> Option<RtcDate> {
        time::get_rtc_date()
    }
}
