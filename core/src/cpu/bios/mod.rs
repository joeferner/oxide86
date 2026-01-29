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
pub mod null_bios;

use super::Cpu;
use crate::{
    DriveNumber,
    cpu::bios::{disk_error::DiskError, dos_error::DosError},
    memory::Memory,
};
pub use int14::{SerialParams, SerialStatus};
pub use int17::PrinterStatus;
pub use int21::FileAccess;
pub use null_bios::NullBios;

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

/// Trait for handling BIOS interrupt I/O operations
/// Platform-specific code (native, WASM) implements this to provide actual I/O
pub trait Bios {
    /// Read a character from standard input (blocking)
    fn read_char(&mut self) -> Option<u8>;

    /// Check if a character is available and return it (non-blocking)
    /// Used by INT 21h, AH=06h (Direct Console I/O)
    fn check_char(&mut self) -> Option<u8>;

    /// Check if a character is available without consuming it
    /// Used by INT 21h, AH=0Bh (Check Input Status)
    fn has_char_available(&self) -> bool;

    // --- INT 16h - Keyboard Services ---

    /// Read a keystroke (INT 16h, AH=00h) - blocking
    /// Returns key press data if available, None otherwise
    fn read_key(&mut self) -> Option<KeyPress>;

    /// Check if a key is available without blocking (INT 16h, AH=01h)
    /// Returns key press data if available, None if no key is waiting
    fn check_key(&mut self) -> Option<KeyPress>;

    // --- INT 13h - Disk Services ---

    /// Reset disk system (INT 13h, AH=00h)
    /// Returns true if successful
    fn disk_reset(&mut self, drive: DriveNumber) -> bool;

    /// Read sectors from disk (INT 13h, AH=02h)
    /// Returns the read data on success, or error code on failure
    fn disk_read_sectors(
        &mut self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, DiskError>;

    /// Write sectors to disk (INT 13h, AH=03h)
    /// Returns number of sectors written on success, or error code on failure
    fn disk_write_sectors(
        &mut self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
        data: &[u8],
    ) -> Result<u8, DiskError>;

    /// Get drive parameters (INT 13h, AH=08h)
    /// Returns drive parameters on success, or error code on failure
    fn disk_get_params(&self, drive: DriveNumber) -> Result<DriveParams, DiskError>;

    /// Get disk type (INT 13h, AH=15h)
    /// Returns (drive_type, sector_count) where:
    /// - drive_type: 0x00 = not present, 0x01 = floppy no change-line,
    ///   0x02 = floppy with change-line, 0x03 = fixed disk
    /// - sector_count: total 512-byte sectors (only for type 0x03)
    fn disk_get_type(&self, drive: DriveNumber) -> Result<(u8, u32), DiskError>;

    /// Detect disk change (INT 13h, AH=16h)
    /// Returns Ok(false) if disk not changed, Ok(true) if disk changed,
    /// or Err(error_code) on error
    fn disk_detect_change(&mut self, drive: DriveNumber) -> Result<bool, DiskError>;

    /// Format track (INT 13h, AH=05h)
    /// Formats a track by filling sectors with zeros
    /// Returns Ok(()) on success, or Err(error_code) on failure
    fn disk_format_track(
        &mut self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sectors_per_track: u8,
    ) -> Result<(), DiskError>;

    /// Read sectors using logical sector addressing (INT 25h)
    /// drive: DOS drive number (0=A, 1=B, 2=C, etc.)
    /// start_sector: starting logical sector number
    /// count: number of sectors to read
    /// Returns the read data on success, or error code on failure
    fn disk_read_sectors_lba(
        &mut self,
        drive: DriveNumber,
        start_sector: u32,
        count: u16,
    ) -> Result<Vec<u8>, DiskError>;

    // --- INT 21h - DOS File Services ---

    /// Create or truncate file (INT 21h, AH=3Ch)
    /// Returns file handle on success, or error code on failure
    fn file_create(&mut self, filename: &str, attributes: u8) -> Result<u16, DosError>;

    /// Open existing file (INT 21h, AH=3Dh)
    /// Returns file handle on success, or error code on failure
    fn file_open(&mut self, filename: &str, access_mode: FileAccess) -> Result<u16, DosError>;

    /// Close file (INT 21h, AH=3Eh)
    /// Returns success or error code
    fn file_close(&mut self, handle: u16) -> Result<(), DosError>;

    /// Read from file or device (INT 21h, AH=3Fh)
    /// Returns the data read on success, or error code on failure
    fn file_read(&mut self, handle: u16, max_bytes: u16) -> Result<Vec<u8>, DosError>;

    /// Write to file or device (INT 21h, AH=40h)
    /// Returns the number of bytes written on success, or error code on failure
    fn file_write(&mut self, handle: u16, data: &[u8]) -> Result<u16, DosError>;

    /// Seek within file (INT 21h, AH=42h)
    /// Returns the new file position on success, or error code on failure
    fn file_seek(&mut self, handle: u16, offset: i32, method: SeekMethod) -> Result<u32, DosError>;

    /// Duplicate file handle (INT 21h, AH=45h)
    /// Returns a new file handle that refers to the same file on success, or error code on failure
    fn file_duplicate(&mut self, handle: u16) -> Result<u16, DosError>;

    // --- INT 21h - DOS Directory Services ---

    /// Create directory (INT 21h, AH=39h)
    /// Returns success or error code
    fn dir_create(&mut self, dirname: &str) -> Result<(), DosError>;

    /// Remove directory (INT 21h, AH=3Ah)
    /// Returns success or error code
    fn dir_remove(&mut self, dirname: &str) -> Result<(), DosError>;

    /// Change current directory (INT 21h, AH=3Bh)
    /// Returns success or error code
    fn dir_change(&mut self, dirname: &str) -> Result<(), DosError>;

    /// Get current directory (INT 21h, AH=47h)
    /// Returns the current directory path (without drive letter)
    fn dir_get_current(&self, drive: DriveNumber) -> Result<String, DosError>;

    /// Find first matching file (INT 21h, AH=4Eh)
    /// Returns file data on success, or error code on failure
    /// The search_id is used to identify this search for subsequent find_next calls
    fn find_first(&mut self, pattern: &str, attributes: u8) -> Result<(usize, FindData), DosError>;

    /// Find next matching file (INT 21h, AH=4Fh)
    /// Returns file data on success, or error code on failure
    fn find_next(&mut self, search_id: usize) -> Result<FindData, DosError>;

    // --- INT 21h - DOS System Functions ---

    /// Get current default drive (INT 21h, AH=19h)
    /// Returns the current drive number (0=A, 1=B, etc.)
    fn get_current_drive(&self) -> DriveNumber;

    /// Set default drive (INT 21h, AH=0Eh)
    /// Returns the total number of logical drives
    fn set_default_drive(&mut self, drive: DriveNumber) -> u8;

    /// Allocate memory (INT 21h, AH=48h)
    /// Returns segment of allocated memory on success, or (error_code, max_available) on failure
    fn memory_allocate(&mut self, paragraphs: u16) -> Result<u16, (DosError, u16)>;

    /// Free memory (INT 21h, AH=49h)
    /// Returns success or error code
    fn memory_free(&mut self, segment: u16) -> Result<(), DosError>;

    /// Resize memory block (INT 21h, AH=4Ah)
    /// Returns success or (error_code, max_available) on failure
    fn memory_resize(&mut self, segment: u16, paragraphs: u16) -> Result<(), (DosError, u16)>;

    /// Get PSP segment (INT 21h, AH=50h/51h/62h)
    /// Returns the current Program Segment Prefix segment
    fn get_psp(&self) -> u16;

    /// Set PSP segment (INT 21h, AH=50h)
    /// Sets the current Program Segment Prefix segment
    fn set_psp(&mut self, segment: u16);

    /// Get device information for IOCTL (INT 21h, AH=44h, AL=00h)
    /// Returns device information word for the given handle
    fn ioctl_get_device_info(&self, handle: u16) -> Result<u16, DosError>;

    /// Set device information for IOCTL (INT 21h, AH=44h, AL=01h)
    /// Sets device information word for the given handle
    fn ioctl_set_device_info(&mut self, handle: u16, info: u16) -> Result<(), DosError>;

    /// Load and/or execute a program (INT 21h, AH=4Bh)
    /// Returns the program data to be loaded into memory on success,
    /// or error code on failure
    fn exec_load_program(&mut self, params: &ExecParams) -> Result<Vec<u8>, DosError>;

    // --- INT 14h - Serial Port Services ---

    /// Initialize serial port (INT 14h, AH=00h)
    /// Returns the port status after initialization
    fn serial_init(&mut self, port: u8, params: SerialParams) -> SerialStatus;

    /// Write character to serial port (INT 14h, AH=01h)
    /// Returns line status (bit 7 set if timeout)
    fn serial_write(&mut self, port: u8, ch: u8) -> u8;

    /// Read character from serial port (INT 14h, AH=02h)
    /// Returns (character, line_status) on success, or line_status with timeout bit on error
    fn serial_read(&mut self, port: u8) -> Result<(u8, u8), u8>;

    /// Get serial port status (INT 14h, AH=03h)
    /// Returns line and modem status
    fn serial_status(&self, port: u8) -> SerialStatus;

    // --- INT 17h - Printer Services ---

    /// Initialize printer port (INT 17h, AH=01h)
    /// Returns the printer status after initialization
    fn printer_init(&mut self, printer: u8) -> PrinterStatus;

    /// Write character to printer (INT 17h, AH=00h)
    /// Returns printer status
    fn printer_write(&mut self, printer: u8, ch: u8) -> PrinterStatus;

    /// Get printer status (INT 17h, AH=02h)
    /// Returns printer status
    fn printer_status(&self, printer: u8) -> PrinterStatus;

    // --- INT 1Ah - Time Services ---

    /// Get system time in BIOS ticks since midnight (INT 1Ah, AH=00h)
    /// Returns the current time in ticks (18.2 Hz timer)
    /// Platform implementations should read the host system time
    fn get_system_ticks(&self) -> u32;

    /// Get Real Time Clock time (INT 1Ah, AH=02h)
    /// Returns current time in decimal format (not BCD - conversion is done by caller)
    /// Returns None if RTC is not available (e.g., on original 8086 systems)
    fn get_rtc_time(&self) -> Option<RtcTime>;

    /// Get Real Time Clock date (INT 1Ah, AH=04h)
    /// Returns current date in decimal format (not BCD - conversion is done by caller)
    /// Returns None if RTC is not available (e.g., on original 8086 systems)
    fn get_rtc_date(&self) -> Option<RtcDate>;
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
    pub(crate) fn handle_bios_interrupt_direct<T: Bios>(
        &mut self,
        int_num: u8,
        memory: &mut Memory,
        io: &mut T,
        video: &mut crate::video::Video,
    ) {
        self.handle_bios_interrupt_impl(int_num, memory, io, video);
    }

    /// Internal implementation of BIOS interrupt handling
    pub(super) fn handle_bios_interrupt_impl<T: Bios>(
        &mut self,
        int_num: u8,
        memory: &mut Memory,
        io: &mut T,
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
