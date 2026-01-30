// BIOS and DOS interrupt handler trait and implementation
// The core provides the interrupt dispatch mechanism, but I/O is handled via callbacks

pub mod disk_error;
pub mod dos_error;
mod int10;
mod int11;
mod int12;
mod int13;
pub mod int14;
mod int15;
mod int16;
pub mod int17;
mod int1a;
mod int20;
mod int21;
mod int25;
mod int28;
mod int29;
mod int2a;
mod int2f;
mod int35_3f;

use super::Cpu;
use crate::{
    DiskController, DriveManager, DriveNumber, KeyboardInput, MemoryAllocator,
    cpu::bios::{disk_error::DiskError, dos_error::DosError},
    memory::Memory,
    peripheral, time,
};
pub use int14::{SerialParams, SerialStatus};
pub use int17::PrinterStatus;
pub use int21::FileAccess;
use std::collections::HashMap;
use std::io::{self, Read};

/// Drive parameters returned by INT 13h, AH=08h
#[derive(Debug, Clone, Copy)]
pub struct DriveParams {
    /// Maximum cylinder number (0-based)
    pub max_cylinder: u8,
    /// Maximum head number (0-based)
    pub max_head: u8,
    /// Maximum sector number (1-based)
    pub max_sector: u8,
    /// Number of drives
    pub drive_count: u8,
}

/// Key press data returned by INT 16h keyboard services
#[derive(Debug, Clone, Copy)]
pub struct KeyPress {
    /// BIOS scan code
    pub scan_code: u8,
    /// ASCII character code
    pub ascii_code: u8,
}

/// RTC (Real Time Clock) time data returned by INT 1Ah, AH=02h
#[derive(Debug, Clone, Copy)]
pub struct RtcTime {
    /// Hours (0-23, decimal not BCD)
    pub hours: u8,
    /// Minutes (0-59, decimal not BCD)
    pub minutes: u8,
    /// Seconds (0-59, decimal not BCD)
    pub seconds: u8,
    /// Daylight saving time flag (0 = standard time, 1 = daylight time)
    pub dst_flag: u8,
}

/// RTC (Real Time Clock) date data returned by INT 1Ah, AH=04h
#[derive(Debug, Clone, Copy)]
pub struct RtcDate {
    /// Century (19 or 20, decimal not BCD)
    pub century: u8,
    /// Year within century (0-99, decimal not BCD)
    pub year: u8,
    /// Month (1-12, decimal not BCD)
    pub month: u8,
    /// Day of month (1-31, decimal not BCD)
    pub day: u8,
}

/// File seek methods for INT 21h, AH=42h
#[derive(Debug, Clone, Copy)]
pub enum SeekMethod {
    /// Seek from beginning of file
    FromStart = 0x00,
    /// Seek from current position
    FromCurrent = 0x01,
    /// Seek from end of file
    FromEnd = 0x02,
}

/// File attributes for DOS
pub mod file_attributes {
    pub const READ_ONLY: u8 = 0x01;
    pub const HIDDEN: u8 = 0x02;
    pub const SYSTEM: u8 = 0x04;
    pub const VOLUME_LABEL: u8 = 0x08;
    pub const DIRECTORY: u8 = 0x10;
    pub const ARCHIVE: u8 = 0x20;
}

/// File information returned by find first/next operations
#[derive(Debug, Clone)]
pub struct FindData {
    /// File attributes
    pub attributes: u8,
    /// File time (DOS packed format)
    pub time: u16,
    /// File date (DOS packed format)
    pub date: u16,
    /// File size in bytes
    pub size: u32,
    /// Filename (null-terminated, 13 bytes max for 8.3 format)
    pub filename: String,
}

/// EXEC function parameters (INT 21h, AH=4Bh)
#[derive(Debug, Clone)]
pub struct ExecParams {
    /// Subfunction (AL value)
    /// 0x00 = Load and execute
    /// 0x01 = Load but don't execute
    /// 0x03 = Load overlay
    pub subfunction: u8,
    /// Program filename
    pub filename: String,
    /// Environment segment (0 = use parent's)
    pub env_segment: u16,
    /// Command line string (without program name)
    pub command_line: String,
}

/// Result of EXEC function for load-only (AL=01h) or overlay (AL=03h)
#[derive(Debug, Clone)]
pub struct ExecResult {
    /// Segment where program was loaded (PSP segment for AL=00h/01h)
    pub load_segment: u16,
    /// Entry point offset (only for AL=01h)
    pub entry_offset: u16,
    /// Entry point segment (only for AL=01h)
    pub entry_segment: u16,
}

/// DOS device types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DosDevice {
    Null,    // NUL device
    Console, // CON device
}

/// Shared BIOS state used by both native CLI and GUI implementations
/// Contains platform-independent components that can be reused across different frontends
pub struct SharedBiosState<D: DiskController> {
    pub drive_manager: DriveManager<D>,
    pub memory_allocator: MemoryAllocator,
    pub device_handles: HashMap<u16, DosDevice>,
    /// Next device handle to allocate. Shared between device handles and file handles.
    pub next_device_handle: u16,
}

impl<D: DiskController> SharedBiosState<D> {
    /// Create a new SharedBiosState with no drives attached
    pub fn new() -> Self {
        Self {
            drive_manager: DriveManager::new(),
            memory_allocator: MemoryAllocator::new(),
            device_handles: HashMap::new(),
            next_device_handle: 3, // 0, 1, 2 are reserved for stdin/stdout/stderr
        }
    }

    /// Check if a filename is a DOS device name
    pub fn is_dos_device(filename: &str) -> Option<DosDevice> {
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
    pub fn allocate_device_handle(&mut self) -> Option<u16> {
        let handle = self.next_device_handle;
        self.next_device_handle = self.next_device_handle.wrapping_add(1);
        if self.next_device_handle < 3 {
            self.next_device_handle = 3; // Wrap around but skip reserved handles
        }
        // Sync the handle counter with the drive manager
        self.drive_manager.set_next_handle(self.next_device_handle);
        Some(handle)
    }
}

impl<D: DiskController> Default for SharedBiosState<D> {
    fn default() -> Self {
        Self::new()
    }
}

/// BIOS implementation for handling interrupt I/O operations
/// Generic over KeyboardInput (for platform-specific keyboard handling) and DiskController
pub struct Bios<K: KeyboardInput, D: DiskController> {
    /// Shared BIOS state (drive manager, memory allocator, device handles)
    pub shared: SharedBiosState<D>,
    /// Keyboard input handler (platform-specific)
    pub keyboard: K,
}

impl<K: KeyboardInput, D: DiskController> Bios<K, D> {
    /// Create a new Bios with the provided keyboard input handler
    pub fn new(keyboard: K) -> Self {
        Self {
            shared: SharedBiosState::new(),
            keyboard,
        }
    }

    /// Insert a floppy disk into a slot (0 = A:, 1 = B:)
    pub fn insert_floppy(&mut self, slot: DriveNumber, disk: D) -> Result<(), String> {
        self.shared.drive_manager.insert_floppy(slot, disk)
    }

    /// Eject a floppy disk from a slot (for runtime disk swapping)
    pub fn eject_floppy(&mut self, slot: DriveNumber) -> Result<Option<D>, String> {
        self.shared.drive_manager.eject_floppy(slot)
    }

    /// Add a hard drive (returns assigned drive number: 0x80, 0x81, etc.)
    pub fn add_hard_drive(&mut self, disk: D) -> DriveNumber {
        self.shared.drive_manager.add_hard_drive(disk)
    }

    /// Add a partitioned hard drive with both partition and raw disk views
    /// This allows INT 13h to read the MBR while DOS file operations use the partition
    pub fn add_hard_drive_with_partition(&mut self, partition: D, raw_disk: D) -> DriveNumber {
        self.shared
            .drive_manager
            .add_hard_drive_with_partition(partition, raw_disk)
    }

    // Console I/O - delegate to keyboard
    pub fn read_char(&mut self) -> Option<u8> {
        self.keyboard.read_char()
    }

    pub fn check_char(&mut self) -> Option<u8> {
        self.keyboard.check_char()
    }

    pub fn has_char_available(&self) -> bool {
        self.keyboard.has_char_available()
    }

    pub fn read_key(&mut self) -> Option<KeyPress> {
        self.keyboard.read_key()
    }

    pub fn check_key(&mut self) -> Option<KeyPress> {
        self.keyboard.check_key()
    }

    // Disk operations - delegate to DriveManager
    pub fn disk_reset(&mut self, drive: DriveNumber) -> bool {
        self.shared.drive_manager.disk_reset(drive)
    }

    pub fn disk_read_sectors(
        &mut self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, DiskError> {
        self.shared
            .drive_manager
            .disk_read_sectors(drive, cylinder, head, sector, count)
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
        self.shared
            .drive_manager
            .disk_write_sectors(drive, cylinder, head, sector, count, data)
    }

    pub fn disk_get_params(&self, drive: DriveNumber) -> Result<DriveParams, DiskError> {
        self.shared.drive_manager.disk_get_params(drive)
    }

    pub fn disk_get_type(&self, drive: DriveNumber) -> Result<(u8, u32), DiskError> {
        self.shared.drive_manager.disk_get_type(drive)
    }

    pub fn disk_detect_change(&mut self, drive: DriveNumber) -> Result<bool, DiskError> {
        self.shared.drive_manager.disk_detect_change(drive)
    }

    pub fn disk_format_track(
        &mut self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sectors_per_track: u8,
    ) -> Result<(), DiskError> {
        self.shared
            .drive_manager
            .disk_format_track(drive, cylinder, head, sectors_per_track)
    }

    pub fn disk_read_sectors_lba(
        &mut self,
        drive: DriveNumber,
        start_sector: u32,
        count: u16,
    ) -> Result<Vec<u8>, DiskError> {
        self.shared
            .drive_manager
            .disk_read_sectors_lba(drive, start_sector, count)
    }

    // File operations
    pub fn file_create(&mut self, filename: &str, attributes: u8) -> Result<u16, DosError> {
        // Check if it's a DOS device
        if let Some(device) = SharedBiosState::<D>::is_dos_device(filename) {
            let handle = self
                .shared
                .allocate_device_handle()
                .ok_or(DosError::TooManyOpenFiles)?;
            self.shared.device_handles.insert(handle, device);
            return Ok(handle);
        }

        // Delegate to drive manager
        self.shared.drive_manager.file_create(filename, attributes)
    }

    pub fn file_open(&mut self, filename: &str, access_mode: FileAccess) -> Result<u16, DosError> {
        // Check if it's a DOS device
        if let Some(device) = SharedBiosState::<D>::is_dos_device(filename) {
            let handle = self
                .shared
                .allocate_device_handle()
                .ok_or(DosError::TooManyOpenFiles)?;
            self.shared.device_handles.insert(handle, device);
            return Ok(handle);
        }

        // Delegate to drive manager
        self.shared.drive_manager.file_open(filename, access_mode)
    }

    pub fn file_close(&mut self, handle: u16) -> Result<(), DosError> {
        // Don't allow closing standard handles
        if handle < 3 {
            return Err(DosError::InvalidHandle);
        }

        // Try removing from device handles first
        if self.shared.device_handles.remove(&handle).is_some() {
            return Ok(());
        }

        // Delegate to drive manager
        self.shared.drive_manager.file_close(handle)
    }

    pub fn file_read(&mut self, handle: u16, max_bytes: u16) -> Result<Vec<u8>, DosError> {
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
        } else if let Some(device) = self.shared.device_handles.get(&handle) {
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
            self.shared.drive_manager.file_read(handle, max_bytes)
        }
    }

    pub fn file_write(&mut self, handle: u16, data: &[u8]) -> Result<u16, DosError> {
        // Note: stdout (1) and stderr (2) are handled by INT 21h, AH=40h
        // which routes them through the video system via teletype output.
        // We should never receive handle 1 or 2 here.
        if handle == 1 || handle == 2 {
            // This shouldn't happen - INT 21h already handles these
            log::warn!("Unexpected direct write to stdout/stderr handle {}", handle);
            return Ok(data.len() as u16);
        }

        if let Some(device) = self.shared.device_handles.get(&handle) {
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
            self.shared.drive_manager.file_write(handle, data)
        }
    }

    pub fn file_seek(
        &mut self,
        handle: u16,
        offset: i32,
        method: SeekMethod,
    ) -> Result<u32, DosError> {
        // Standard handles and device handles don't support seeking
        if handle < 3 || self.shared.device_handles.contains_key(&handle) {
            return Err(DosError::InvalidHandle);
        }

        // Delegate to drive manager
        self.shared.drive_manager.file_seek(handle, offset, method)
    }

    pub fn file_duplicate(&mut self, handle: u16) -> Result<u16, DosError> {
        // Standard handles (0, 1, 2) can be duplicated
        if handle < 3 {
            let new_handle = self
                .shared
                .allocate_device_handle()
                .ok_or(DosError::TooManyOpenFiles)?;
            // We don't actually store anything for standard handles
            return Ok(new_handle);
        }

        // Check if it's a device handle
        if let Some(device) = self.shared.device_handles.get(&handle).copied() {
            let new_handle = self
                .shared
                .allocate_device_handle()
                .ok_or(DosError::TooManyOpenFiles)?;
            self.shared.device_handles.insert(new_handle, device);
            return Ok(new_handle);
        }

        // Delegate to drive manager
        self.shared.drive_manager.file_duplicate(handle)
    }

    // Directory operations
    pub fn dir_create(&mut self, dirname: &str) -> Result<(), DosError> {
        self.shared.drive_manager.dir_create(dirname)
    }

    pub fn dir_remove(&mut self, dirname: &str) -> Result<(), DosError> {
        self.shared.drive_manager.dir_remove(dirname)
    }

    pub fn dir_change(&mut self, dirname: &str) -> Result<(), DosError> {
        self.shared.drive_manager.dir_change(dirname)
    }

    pub fn dir_get_current(&self, drive: DriveNumber) -> Result<String, DosError> {
        self.shared.drive_manager.get_current_dir(drive)
    }

    pub fn find_first(
        &mut self,
        pattern: &str,
        attributes: u8,
    ) -> Result<(usize, FindData), DosError> {
        self.shared.drive_manager.find_first(pattern, attributes)
    }

    pub fn find_next(&mut self, search_id: usize) -> Result<FindData, DosError> {
        self.shared.drive_manager.find_next(search_id)
    }

    // Drive management
    pub fn get_current_drive(&self) -> DriveNumber {
        self.shared.drive_manager.get_current_drive()
    }

    pub fn set_default_drive(&mut self, drive: DriveNumber) -> u8 {
        match self.shared.drive_manager.set_current_drive(drive) {
            Ok(count) => count,
            Err(_) => self.shared.drive_manager.get_drive_count(),
        }
    }

    // Memory management
    pub fn memory_allocate(&mut self, paragraphs: u16) -> Result<u16, (DosError, u16)> {
        self.shared.memory_allocator.allocate(paragraphs)
    }

    pub fn memory_free(&mut self, segment: u16) -> Result<(), DosError> {
        self.shared.memory_allocator.free(segment)
    }

    pub fn memory_resize(&mut self, segment: u16, paragraphs: u16) -> Result<(), (DosError, u16)> {
        self.shared.memory_allocator.resize(segment, paragraphs)
    }

    // PSP management
    pub fn get_psp(&self) -> u16 {
        // Default PSP segment for simple programs
        0x0100
    }

    pub fn set_psp(&mut self, _segment: u16) {
        // PSP tracking is not implemented in this simple BIOS
    }

    // IOCTL
    pub fn ioctl_get_device_info(&self, handle: u16) -> Result<u16, DosError> {
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
                if let Some(device) = self.shared.device_handles.get(&handle) {
                    // Return device info based on device type
                    match device {
                        DosDevice::Null => Ok(0x8004),    // NUL device (bit 7), special device
                        DosDevice::Console => Ok(0x80D3), // CON device (bit 7), console I/O
                    }
                } else if self.shared.drive_manager.contains_handle(handle) {
                    // It's a regular file (bit 7 = 0)
                    Ok(0x0000)
                } else {
                    Err(DosError::InvalidHandle)
                }
            }
        }
    }

    pub fn ioctl_set_device_info(&mut self, handle: u16, _info: u16) -> Result<(), DosError> {
        // Allow setting device info for standard handles and open files
        match handle {
            0..=2 => {
                // Standard handles - allow setting but ignore
                Ok(())
            }
            _ => {
                // Check if it's a valid file handle
                if self.shared.drive_manager.contains_handle(handle)
                    || self.shared.device_handles.contains_key(&handle)
                {
                    // Allow setting but ignore for files and devices
                    Ok(())
                } else {
                    Err(DosError::InvalidHandle)
                }
            }
        }
    }

    pub fn exec_load_program(&mut self, params: &ExecParams) -> Result<Vec<u8>, DosError> {
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
    pub fn serial_init(&mut self, port: u8, params: SerialParams) -> SerialStatus {
        peripheral::serial_init(port, params)
    }

    pub fn serial_write(&mut self, port: u8, ch: u8) -> u8 {
        peripheral::serial_write(port, ch)
    }

    pub fn serial_read(&mut self, port: u8) -> Result<(u8, u8), u8> {
        peripheral::serial_read(port)
    }

    pub fn serial_status(&self, port: u8) -> SerialStatus {
        peripheral::serial_status(port)
    }

    // Printer (stub implementations)
    pub fn printer_init(&mut self, printer: u8) -> PrinterStatus {
        peripheral::printer_init(printer)
    }

    pub fn printer_write(&mut self, printer: u8, ch: u8) -> PrinterStatus {
        peripheral::printer_write(printer, ch)
    }

    pub fn printer_status(&self, printer: u8) -> PrinterStatus {
        peripheral::printer_status(printer)
    }

    // Time and RTC
    pub fn get_system_ticks(&self) -> u32 {
        time::get_system_ticks()
    }

    pub fn get_rtc_time(&self) -> Option<RtcTime> {
        time::get_rtc_time()
    }

    pub fn get_rtc_date(&self) -> Option<RtcDate> {
        time::get_rtc_date()
    }
}

impl Cpu {
    /// Check if an interrupt vector still points to the BIOS area (F000 segment)
    /// Returns true if the vector is in BIOS ROM area, false if DOS has installed its own handler
    pub(super) fn is_bios_handler(memory: &Memory, int_num: u8) -> bool {
        let ivt_addr = (int_num as usize) * 4;
        let segment = memory.read_u16(ivt_addr + 2);
        segment == 0xF000 // BIOS handlers are in the F000 segment (ROM area)
    }

    /// Handle BIOS interrupt directly without checking IVT
    /// Used when DOS chains back to BIOS via CALL FAR to F000:XXXX
    pub(crate) fn handle_bios_interrupt_direct<K: KeyboardInput, D: DiskController>(
        &mut self,
        int_num: u8,
        memory: &mut Memory,
        io: &mut Bios<K, D>,
        video: &mut crate::video::Video,
    ) {
        self.handle_bios_interrupt_impl(int_num, memory, io, video);
    }

    /// Internal implementation of BIOS interrupt handling
    pub(super) fn handle_bios_interrupt_impl<K: KeyboardInput, D: DiskController>(
        &mut self,
        int_num: u8,
        memory: &mut Memory,
        io: &mut Bios<K, D>,
        video: &mut crate::video::Video,
    ) {
        match int_num {
            0x10 => self.handle_int10(memory, video),
            0x11 => self.handle_int11(memory),
            0x12 => self.handle_int12(memory),
            0x13 => self.handle_int13(memory, io),
            0x14 => self.handle_int14(memory, io),
            0x15 => self.handle_int15(memory, io),
            0x16 => self.handle_int16(memory, io),
            0x17 => self.handle_int17(memory, io),
            0x1A => self.handle_int1a(memory, io),
            0x20 => self.handle_int20(memory, io),
            0x21 => self.handle_int21(memory, io, video),
            0x25 => self.handle_int25(memory, io),
            0x28 => self.handle_int28(),
            0x29 => self.handle_int29(video),
            0x2A => self.handle_int2a(),
            0x2F => self.handle_int2f(memory, io),
            0x35..=0x3F => self.handle_int35_3f(int_num),
            // Other BIOS interrupts can be added here
            _ => {
                log::warn!("Unhandled BIOS interrupt: 0x{:02X}", int_num);
            }
        }
    }
}
