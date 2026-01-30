mod console;
mod peripheral;

use emu86_core::cpu::bios::disk_error::DiskError;
use emu86_core::cpu::bios::dos_error::DosError;
use emu86_core::cpu::bios::{
    DriveParams, ExecParams, FileAccess, FindData, KeyPress, PrinterStatus, RtcDate, RtcTime,
    SeekMethod, SerialParams, SerialStatus,
};
use emu86_core::time;
use emu86_core::{Bios, DiskController, DriveManager, DriveNumber, MemoryAllocator};
use std::collections::{HashMap, VecDeque};
use std::io::{self, Read};

use crate::bios::console::SCAN_CODE_F12;

/// DOS device types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DosDevice {
    Null,    // NUL device
    Console, // CON device
}

/// Native platform implementation of BIOS
pub struct NativeBios<D: DiskController> {
    drive_manager: DriveManager<D>,
    memory_allocator: MemoryAllocator,
    device_handles: HashMap<u16, DosDevice>,
    /// Next device handle to allocate. Shared between device handles and file handles.
    next_device_handle: u16,
    /// Flag set when F12 is pressed (command mode request)
    command_mode_requested: bool,
    /// Buffer for keyboard input read during polling (not F12)
    keyboard_buffer: VecDeque<KeyPress>,
}

impl<D: DiskController> NativeBios<D> {
    /// Create a new NativeBios with no drives attached
    pub fn new() -> Self {
        Self {
            drive_manager: DriveManager::new(),
            memory_allocator: MemoryAllocator::new(),
            device_handles: HashMap::new(),
            next_device_handle: 3, // 0, 1, 2 are reserved for stdin/stdout/stderr
            command_mode_requested: false,
            keyboard_buffer: VecDeque::new(),
        }
    }

    /// Insert a floppy disk into a slot (0 = A:, 1 = B:)
    pub fn insert_floppy(&mut self, slot: DriveNumber, disk: D) -> Result<(), String> {
        self.drive_manager.insert_floppy(slot, disk)
    }

    /// Eject a floppy disk from a slot (for runtime disk swapping)
    #[allow(dead_code)]
    pub fn eject_floppy(&mut self, slot: DriveNumber) -> Result<Option<D>, String> {
        self.drive_manager.eject_floppy(slot)
    }

    /// Add a hard drive (returns assigned drive number: 0x80, 0x81, etc.)
    pub fn add_hard_drive(&mut self, disk: D) -> DriveNumber {
        self.drive_manager.add_hard_drive(disk)
    }

    /// Add a partitioned hard drive with both partition and raw disk views
    /// This allows INT 13h to read the MBR while DOS file operations use the partition
    pub fn add_hard_drive_with_partition(&mut self, partition: D, raw_disk: D) -> DriveNumber {
        self.drive_manager
            .add_hard_drive_with_partition(partition, raw_disk)
    }

    /// Check if a filename is a DOS device name
    fn is_dos_device(filename: &str) -> Option<DosDevice> {
        // DOS device names are case-insensitive and may have extensions
        let name = filename.to_uppercase();
        let base_name = name.split('.').next().unwrap_or(&name);
        // Also strip any path prefix
        let base_name = base_name.rsplit(['\\', '/']).next().unwrap_or(base_name);

        match base_name {
            "NUL" => Some(DosDevice::Null),
            "CON" => Some(DosDevice::Console),
            "AUX" | "COM1" | "COM2" | "COM3" | "COM4" => Some(DosDevice::Null), // Serial ports - stub as null
            "PRN" | "LPT1" | "LPT2" | "LPT3" => Some(DosDevice::Null), // Printer ports - stub as null
            _ => None,
        }
    }

    /// Allocate a new device handle
    /// This is shared between device handles and drive_manager file handles to avoid collisions
    fn allocate_device_handle(&mut self) -> Option<u16> {
        let handle = self.next_device_handle;
        self.next_device_handle = self.next_device_handle.wrapping_add(1);
        if self.next_device_handle < 3 {
            self.next_device_handle = 3; // Wrap around but skip reserved handles
        }
        // Sync the handle counter with the drive manager
        self.drive_manager.set_next_handle(self.next_device_handle);
        Some(handle)
    }

    /// Check if command mode has been requested via F12
    pub fn is_command_mode_requested(&self) -> bool {
        self.command_mode_requested
    }

    /// Clear the command mode request flag
    pub fn clear_command_mode_request(&mut self) {
        self.command_mode_requested = false;
    }

    /// Poll for F12 key press without blocking
    /// This is called from the main loop to detect command mode requests
    /// even when the emulated program doesn't call keyboard BIOS functions.
    /// Keys other than F12 are buffered for later retrieval by BIOS functions.
    pub fn poll_for_command_key(&mut self) {
        // Drain all available keys from the terminal
        while let Some(key) = console::check_key() {
            if key.scan_code == SCAN_CODE_F12 {
                // F12 - set command mode flag and don't buffer it
                self.command_mode_requested = true;
                break; // Stop processing once F12 is detected
            } else {
                // Not F12 - buffer it for later retrieval by BIOS functions
                self.keyboard_buffer.push_back(key);
            }
        }
    }
}

impl<D: DiskController> Default for NativeBios<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D: DiskController> Bios for NativeBios<D> {
    // Console I/O
    fn read_char(&mut self) -> Option<u8> {
        console::read_char()
    }

    fn check_char(&mut self) -> Option<u8> {
        console::check_char()
    }

    fn has_char_available(&self) -> bool {
        console::has_char_available()
    }

    fn read_key(&mut self) -> Option<KeyPress> {
        // First check if we have a buffered key (from poll_for_command_key)
        if let Some(key) = self.keyboard_buffer.pop_front() {
            return Some(key);
        }

        // No buffered key, read from terminal (blocking)
        let key = console::read_key()?;
        // Intercept F12 for command mode
        if key.scan_code == SCAN_CODE_F12 {
            self.command_mode_requested = true;
            // Return None so the emulated program doesn't see F12
            return None;
        }
        Some(key)
    }

    fn check_key(&mut self) -> Option<KeyPress> {
        // First check if we have a buffered key (from poll_for_command_key)
        // Note: We remove it here to prevent infinite re-detection
        if let Some(key) = self.keyboard_buffer.pop_front() {
            return Some(key);
        }

        // No buffered key, check terminal (non-blocking)
        let key = console::check_key()?;
        // Intercept F12 for command mode
        if key.scan_code == SCAN_CODE_F12 {
            self.command_mode_requested = true;
            // Return None so the emulated program doesn't see F12
            return None;
        }
        Some(key)
    }

    // Disk operations - delegate to DriveManager
    fn disk_reset(&mut self, drive: DriveNumber) -> bool {
        self.drive_manager.disk_reset(drive)
    }

    fn disk_read_sectors(
        &mut self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, DiskError> {
        self.drive_manager
            .disk_read_sectors(drive, cylinder, head, sector, count)
    }

    fn disk_write_sectors(
        &mut self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
        data: &[u8],
    ) -> Result<u8, DiskError> {
        self.drive_manager
            .disk_write_sectors(drive, cylinder, head, sector, count, data)
    }

    fn disk_get_params(&self, drive: DriveNumber) -> Result<DriveParams, DiskError> {
        self.drive_manager.disk_get_params(drive)
    }

    fn disk_get_type(&self, drive: DriveNumber) -> Result<(u8, u32), DiskError> {
        self.drive_manager.disk_get_type(drive)
    }

    fn disk_detect_change(&mut self, drive: DriveNumber) -> Result<bool, DiskError> {
        self.drive_manager.disk_detect_change(drive)
    }

    fn disk_format_track(
        &mut self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sectors_per_track: u8,
    ) -> Result<(), DiskError> {
        self.drive_manager
            .disk_format_track(drive, cylinder, head, sectors_per_track)
    }

    fn disk_read_sectors_lba(
        &mut self,
        drive: DriveNumber,
        start_sector: u32,
        count: u16,
    ) -> Result<Vec<u8>, DiskError> {
        self.drive_manager
            .disk_read_sectors_lba(drive, start_sector, count)
    }

    // File operations
    fn file_create(&mut self, filename: &str, attributes: u8) -> Result<u16, DosError> {
        // Check if it's a DOS device
        if let Some(device) = Self::is_dos_device(filename) {
            let handle = self
                .allocate_device_handle()
                .ok_or(DosError::TooManyOpenFiles)?;
            self.device_handles.insert(handle, device);
            return Ok(handle);
        }

        // Delegate to drive manager
        self.drive_manager.file_create(filename, attributes)
    }

    fn file_open(&mut self, filename: &str, access_mode: FileAccess) -> Result<u16, DosError> {
        // Check if it's a DOS device
        if let Some(device) = Self::is_dos_device(filename) {
            let handle = self
                .allocate_device_handle()
                .ok_or(DosError::TooManyOpenFiles)?;
            self.device_handles.insert(handle, device);
            return Ok(handle);
        }

        // Delegate to drive manager
        self.drive_manager.file_open(filename, access_mode)
    }

    fn file_close(&mut self, handle: u16) -> Result<(), DosError> {
        // Don't allow closing standard handles
        if handle < 3 {
            return Err(DosError::InvalidHandle);
        }

        // Try removing from device handles first
        if self.device_handles.remove(&handle).is_some() {
            return Ok(());
        }

        // Delegate to drive manager
        self.drive_manager.file_close(handle)
    }

    fn file_read(&mut self, handle: u16, max_bytes: u16) -> Result<Vec<u8>, DosError> {
        // Handle stdin separately
        if handle == 0 {
            let mut buffer = vec![0u8; max_bytes as usize];
            match io::stdin().read(&mut buffer) {
                Ok(n) => {
                    buffer.truncate(n);
                    Ok(buffer)
                }
                Err(_) => Err(DosError::AccessDenied),
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
                        Err(_) => Err(DosError::AccessDenied),
                    }
                }
            }
        } else {
            // Delegate to drive manager
            self.drive_manager.file_read(handle, max_bytes)
        }
    }

    fn file_write(&mut self, handle: u16, data: &[u8]) -> Result<u16, DosError> {
        // Note: stdout (1) and stderr (2) are handled by INT 21h, AH=40h
        // which routes them through the video system via teletype output.
        // We should never receive handle 1 or 2 here.
        if handle == 1 || handle == 2 {
            // This shouldn't happen - INT 21h already handles these
            log::warn!("Unexpected direct write to stdout/stderr handle {}", handle);
            return Ok(data.len() as u16);
        }

        if let Some(device) = self.device_handles.get(&handle) {
            // Handle DOS devices
            match device {
                DosDevice::Null => {
                    // NUL discards all data but reports success
                    Ok(data.len() as u16)
                }
                DosDevice::Console => {
                    // CON device writes should also go through video system
                    // For now, just report success without output
                    // TODO: Route CON writes through video in INT 21h handler
                    log::warn!("CON device write not yet routed through video system");
                    Ok(data.len() as u16)
                }
            }
        } else {
            // Delegate to drive manager
            self.drive_manager.file_write(handle, data)
        }
    }

    fn file_seek(&mut self, handle: u16, offset: i32, method: SeekMethod) -> Result<u32, DosError> {
        // Standard handles and device handles don't support seeking
        if handle < 3 || self.device_handles.contains_key(&handle) {
            return Err(DosError::InvalidHandle);
        }

        // Delegate to drive manager
        self.drive_manager.file_seek(handle, offset, method)
    }

    fn file_duplicate(&mut self, handle: u16) -> Result<u16, DosError> {
        // Standard handles (0, 1, 2) can be duplicated
        if handle < 3 {
            let new_handle = self
                .allocate_device_handle()
                .ok_or(DosError::TooManyOpenFiles)?;
            // We don't actually store anything for standard handles
            return Ok(new_handle);
        }

        // Check if it's a device handle
        if let Some(device) = self.device_handles.get(&handle).copied() {
            let new_handle = self
                .allocate_device_handle()
                .ok_or(DosError::TooManyOpenFiles)?;
            self.device_handles.insert(new_handle, device);
            return Ok(new_handle);
        }

        // Delegate to drive manager
        self.drive_manager.file_duplicate(handle)
    }

    // Directory operations
    fn dir_create(&mut self, dirname: &str) -> Result<(), DosError> {
        self.drive_manager.dir_create(dirname)
    }

    fn dir_remove(&mut self, dirname: &str) -> Result<(), DosError> {
        self.drive_manager.dir_remove(dirname)
    }

    fn dir_change(&mut self, dirname: &str) -> Result<(), DosError> {
        self.drive_manager.dir_change(dirname)
    }

    fn dir_get_current(&self, drive: DriveNumber) -> Result<String, DosError> {
        self.drive_manager.get_current_dir(drive)
    }

    fn find_first(&mut self, pattern: &str, attributes: u8) -> Result<(usize, FindData), DosError> {
        self.drive_manager.find_first(pattern, attributes)
    }

    fn find_next(&mut self, search_id: usize) -> Result<FindData, DosError> {
        self.drive_manager.find_next(search_id)
    }

    // Drive management
    fn get_current_drive(&self) -> DriveNumber {
        self.drive_manager.get_current_drive()
    }

    fn set_default_drive(&mut self, drive: DriveNumber) -> u8 {
        match self.drive_manager.set_current_drive(drive) {
            Ok(count) => count,
            Err(_) => self.drive_manager.get_drive_count(),
        }
    }

    // Memory management
    fn memory_allocate(&mut self, paragraphs: u16) -> Result<u16, (DosError, u16)> {
        self.memory_allocator.allocate(paragraphs)
    }

    fn memory_free(&mut self, segment: u16) -> Result<(), DosError> {
        self.memory_allocator.free(segment)
    }

    fn memory_resize(&mut self, segment: u16, paragraphs: u16) -> Result<(), (DosError, u16)> {
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
    fn ioctl_get_device_info(&self, handle: u16) -> Result<u16, DosError> {
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
                } else if self.drive_manager.contains_handle(handle) {
                    // It's a regular file (bit 7 = 0)
                    Ok(0x0000)
                } else {
                    Err(DosError::InvalidHandle)
                }
            }
        }
    }

    fn ioctl_set_device_info(&mut self, handle: u16, _info: u16) -> Result<(), DosError> {
        // Allow setting device info for standard handles and open files
        match handle {
            0..=2 => {
                // Standard handles - allow setting but ignore
                Ok(())
            }
            _ => {
                // Check if it's a valid file handle
                if self.drive_manager.contains_handle(handle)
                    || self.device_handles.contains_key(&handle)
                {
                    // Allow setting but ignore for files and devices
                    Ok(())
                } else {
                    Err(DosError::InvalidHandle)
                }
            }
        }
    }

    fn exec_load_program(&mut self, params: &ExecParams) -> Result<Vec<u8>, DosError> {
        // Open the program file
        let handle = self.file_open(&params.filename, FileAccess::ReadOnly)?;

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
            return Err(DosError::FileNotFound);
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
