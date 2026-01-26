mod console;
mod directory;
mod disk;
mod file;
mod memory_allocator;
mod peripheral;
mod time;

use emu86_core::cpu::bios::{
    DriveParams, FindData, KeyPress, PrinterStatus, RtcDate, RtcTime, SeekMethod, SerialParams,
    SerialStatus, dos_errors,
};
use emu86_core::{Bios, DiskController};
use std::path::Path;

use crate::bios::directory::DirectoryManager;
use crate::bios::file::FileManager;
use crate::bios::memory_allocator::MemoryAllocator;

/// Native platform implementation of BIOS
pub struct NativeBios<D: DiskController> {
    disk: D,
    file_manager: FileManager,
    directory_manager: DirectoryManager,
    memory_allocator: MemoryAllocator,
}

impl<D: DiskController> NativeBios<D> {
    pub fn new(disk: D, working_dir: impl AsRef<Path>) -> Self {
        let working_dir = working_dir.as_ref();
        Self {
            disk,
            file_manager: FileManager::new(working_dir),
            directory_manager: DirectoryManager::new(working_dir),
            memory_allocator: MemoryAllocator::new(),
        }
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

    // Disk operations
    fn disk_reset(&mut self, drive: u8) -> bool {
        disk::disk_reset(&mut self.disk, drive)
    }

    fn disk_read_sectors(
        &mut self,
        drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, u8> {
        disk::disk_read_sectors(&mut self.disk, drive, cylinder, head, sector, count)
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
        disk::disk_write_sectors(&mut self.disk, drive, cylinder, head, sector, count, data)
    }

    fn disk_get_params(&self, drive: u8) -> Result<DriveParams, u8> {
        disk::disk_get_params(&self.disk, drive)
    }

    fn disk_get_type(&self, drive: u8) -> Result<(u8, u32), u8> {
        disk::disk_get_type(&self.disk, drive)
    }

    // File operations
    fn file_create(&mut self, filename: &str, attributes: u8) -> Result<u16, u8> {
        self.file_manager.create(filename, attributes)
    }

    fn file_open(&mut self, filename: &str, access_mode: u8) -> Result<u16, u8> {
        self.file_manager.open(filename, access_mode)
    }

    fn file_close(&mut self, handle: u16) -> Result<(), u8> {
        self.file_manager.close(handle)
    }

    fn file_read(&mut self, handle: u16, max_bytes: u16) -> Result<Vec<u8>, u8> {
        self.file_manager.read(handle, max_bytes)
    }

    fn file_write(&mut self, handle: u16, data: &[u8]) -> Result<u16, u8> {
        self.file_manager.write(handle, data)
    }

    fn file_seek(&mut self, handle: u16, offset: i32, method: SeekMethod) -> Result<u32, u8> {
        self.file_manager.seek(handle, offset, method)
    }

    fn file_duplicate(&mut self, handle: u16) -> Result<u16, u8> {
        self.file_manager.duplicate(handle)
    }

    // Directory operations
    fn dir_create(&mut self, dirname: &str) -> Result<(), u8> {
        self.directory_manager.create(dirname)
    }

    fn dir_remove(&mut self, dirname: &str) -> Result<(), u8> {
        self.directory_manager.remove(dirname)
    }

    fn dir_change(&mut self, dirname: &str) -> Result<(), u8> {
        // Update both file and directory managers
        self.directory_manager.change(dirname)?;
        let new_dir = self.directory_manager.working_dir().to_path_buf();
        self.file_manager.set_working_dir(new_dir);
        Ok(())
    }

    fn dir_get_current(&self, drive: u8) -> Result<String, u8> {
        self.directory_manager.get_current(drive)
    }

    fn find_first(&mut self, pattern: &str, attributes: u8) -> Result<(usize, FindData), u8> {
        self.directory_manager.find_first(pattern, attributes)
    }

    fn find_next(&mut self, search_id: usize) -> Result<FindData, u8> {
        self.directory_manager.find_next(search_id)
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
                if let Some(device) = self.file_manager.get_device(handle) {
                    // Return device info based on device type
                    match device {
                        file::DosDevice::Null => Ok(0x8004), // NUL device (bit 7), special device
                        file::DosDevice::Console => Ok(0x80D3), // CON device (bit 7), console I/O
                    }
                } else if self.file_manager.contains_handle(handle) {
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
                if self.file_manager.contains_handle(handle) {
                    // Allow setting but ignore for files
                    Ok(())
                } else {
                    Err(dos_errors::INVALID_HANDLE)
                }
            }
        }
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
