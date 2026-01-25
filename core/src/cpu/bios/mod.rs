// BIOS and DOS interrupt handler trait and implementation
// The core provides the interrupt dispatch mechanism, but I/O is handled via callbacks

mod int10;
mod int11;
mod int12;
mod int13;
pub mod int14;
mod int15;
mod int16;
pub mod int17;
mod int1a;
mod int21;
mod int29;
mod int2f;
pub mod null_bios;

use super::Cpu;
use crate::memory::Memory;
use log::warn;
pub use null_bios::NullBios;
pub use int14::{SerialParams, SerialStatus};
pub use int17::PrinterStatus;

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

/// INT 13h error codes
pub mod disk_errors {
    pub const SUCCESS: u8 = 0x00;
    pub const INVALID_COMMAND: u8 = 0x01;
    pub const ADDRESS_MARK_NOT_FOUND: u8 = 0x02;
    pub const WRITE_PROTECTED: u8 = 0x03;
    pub const SECTOR_NOT_FOUND: u8 = 0x04;
    pub const RESET_FAILED: u8 = 0x05;
    pub const DISK_CHANGED: u8 = 0x06;
    pub const DRIVE_PARAMETER_ACTIVITY_FAILED: u8 = 0x07;
    pub const DMA_OVERRUN: u8 = 0x08;
    pub const DMA_BOUNDARY_ERROR: u8 = 0x09;
    pub const BAD_SECTOR: u8 = 0x0A;
    pub const BAD_TRACK: u8 = 0x0B;
    pub const UNSUPPORTED_TRACK: u8 = 0x0C;
    pub const INVALID_NUMBER_OF_SECTORS: u8 = 0x0D;
    pub const CONTROL_DATA_ADDRESS_MARK_DETECTED: u8 = 0x0E;
    pub const DMA_ARBITRATION_LEVEL_OUT_OF_RANGE: u8 = 0x0F;
    pub const UNCORRECTABLE_CRC_ERROR: u8 = 0x10;
    pub const ECC_CORRECTED_DATA_ERROR: u8 = 0x11;
    pub const CONTROLLER_FAILURE: u8 = 0x20;
    pub const SEEK_FAILED: u8 = 0x40;
    pub const TIMEOUT: u8 = 0x80;
    pub const DRIVE_NOT_READY: u8 = 0xAA;
    pub const UNDEFINED_ERROR: u8 = 0xBB;
    pub const WRITE_FAULT: u8 = 0xCC;
    pub const STATUS_REGISTER_ERROR: u8 = 0xE0;
    pub const SENSE_OPERATION_FAILED: u8 = 0xFF;
}

/// INT 21h DOS error codes
pub mod dos_errors {
    pub const SUCCESS: u8 = 0x00;
    pub const INVALID_FUNCTION: u8 = 0x01;
    pub const FILE_NOT_FOUND: u8 = 0x02;
    pub const PATH_NOT_FOUND: u8 = 0x03;
    pub const TOO_MANY_OPEN_FILES: u8 = 0x04;
    pub const ACCESS_DENIED: u8 = 0x05;
    pub const INVALID_HANDLE: u8 = 0x06;
    pub const MEMORY_CONTROL_BLOCKS_DESTROYED: u8 = 0x07;
    pub const INSUFFICIENT_MEMORY: u8 = 0x08;
    pub const INVALID_MEMORY_BLOCK_ADDRESS: u8 = 0x09;
    pub const INVALID_ENVIRONMENT: u8 = 0x0A;
    pub const INVALID_FORMAT: u8 = 0x0B;
    pub const INVALID_ACCESS_CODE: u8 = 0x0C;
    pub const INVALID_DATA: u8 = 0x0D;
    pub const INVALID_DRIVE: u8 = 0x0F;
    pub const ATTEMPT_TO_REMOVE_CURRENT_DIR: u8 = 0x10;
    pub const NOT_SAME_DEVICE: u8 = 0x11;
    pub const NO_MORE_FILES: u8 = 0x12;
}

/// File access modes for INT 21h, AH=3Dh
pub mod file_access {
    pub const READ_ONLY: u8 = 0x00;
    pub const WRITE_ONLY: u8 = 0x01;
    pub const READ_WRITE: u8 = 0x02;
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

/// Trait for handling BIOS interrupt I/O operations
/// Platform-specific code (native, WASM) implements this to provide actual I/O
pub trait Bios {
    /// Read a character from standard input
    fn read_char(&mut self) -> Option<u8>;

    /// Write a character to standard output
    fn write_char(&mut self, ch: u8);

    /// Write a string to standard output
    fn write_str(&mut self, s: &str);

    // --- INT 16h - Keyboard Services ---

    /// Read a keystroke (INT 16h, AH=00h)
    /// Returns key press data if available, None otherwise
    fn read_key(&mut self) -> Option<KeyPress>;

    // --- INT 13h - Disk Services ---

    /// Reset disk system (INT 13h, AH=00h)
    /// Returns true if successful
    fn disk_reset(&mut self, drive: u8) -> bool;

    /// Read sectors from disk (INT 13h, AH=02h)
    /// Returns the read data on success, or error code on failure
    fn disk_read_sectors(
        &mut self,
        drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, u8>;

    /// Write sectors to disk (INT 13h, AH=03h)
    /// Returns number of sectors written on success, or error code on failure
    fn disk_write_sectors(
        &mut self,
        drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
        data: &[u8],
    ) -> Result<u8, u8>;

    /// Get drive parameters (INT 13h, AH=08h)
    /// Returns drive parameters on success, or error code on failure
    fn disk_get_params(&self, drive: u8) -> Result<DriveParams, u8>;

    /// Get disk type (INT 13h, AH=15h)
    /// Returns (drive_type, sector_count) where:
    /// - drive_type: 0x00 = not present, 0x01 = floppy no change-line,
    ///   0x02 = floppy with change-line, 0x03 = fixed disk
    /// - sector_count: total 512-byte sectors (only for type 0x03)
    fn disk_get_type(&self, drive: u8) -> Result<(u8, u32), u8>;

    // --- INT 21h - DOS File Services ---

    /// Create or truncate file (INT 21h, AH=3Ch)
    /// Returns file handle on success, or error code on failure
    fn file_create(&mut self, filename: &str, attributes: u8) -> Result<u16, u8>;

    /// Open existing file (INT 21h, AH=3Dh)
    /// Returns file handle on success, or error code on failure
    fn file_open(&mut self, filename: &str, access_mode: u8) -> Result<u16, u8>;

    /// Close file (INT 21h, AH=3Eh)
    /// Returns success or error code
    fn file_close(&mut self, handle: u16) -> Result<(), u8>;

    /// Read from file or device (INT 21h, AH=3Fh)
    /// Returns the data read on success, or error code on failure
    fn file_read(&mut self, handle: u16, max_bytes: u16) -> Result<Vec<u8>, u8>;

    /// Write to file or device (INT 21h, AH=40h)
    /// Returns the number of bytes written on success, or error code on failure
    fn file_write(&mut self, handle: u16, data: &[u8]) -> Result<u16, u8>;

    /// Seek within file (INT 21h, AH=42h)
    /// Returns the new file position on success, or error code on failure
    fn file_seek(&mut self, handle: u16, offset: i32, method: SeekMethod) -> Result<u32, u8>;

    // --- INT 21h - DOS Directory Services ---

    /// Create directory (INT 21h, AH=39h)
    /// Returns success or error code
    fn dir_create(&mut self, dirname: &str) -> Result<(), u8>;

    /// Remove directory (INT 21h, AH=3Ah)
    /// Returns success or error code
    fn dir_remove(&mut self, dirname: &str) -> Result<(), u8>;

    /// Change current directory (INT 21h, AH=3Bh)
    /// Returns success or error code
    fn dir_change(&mut self, dirname: &str) -> Result<(), u8>;

    /// Get current directory (INT 21h, AH=47h)
    /// Returns the current directory path (without drive letter)
    fn dir_get_current(&self, drive: u8) -> Result<String, u8>;

    /// Find first matching file (INT 21h, AH=4Eh)
    /// Returns file data on success, or error code on failure
    /// The search_id is used to identify this search for subsequent find_next calls
    fn find_first(&mut self, pattern: &str, attributes: u8) -> Result<(usize, FindData), u8>;

    /// Find next matching file (INT 21h, AH=4Fh)
    /// Returns file data on success, or error code on failure
    fn find_next(&mut self, search_id: usize) -> Result<FindData, u8>;

    // --- INT 21h - DOS System Functions ---

    /// Get current default drive (INT 21h, AH=19h)
    /// Returns the current drive number (0=A, 1=B, etc.)
    fn get_current_drive(&self) -> u8;

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
}

impl Cpu {
    /// Handle BIOS/DOS interrupts with provided I/O handler
    /// Returns true if the interrupt was handled, false if it should proceed normally
    pub(super) fn handle_bios_interrupt<T: Bios>(
        &mut self,
        int_num: u8,
        memory: &mut Memory,
        io: &mut T,
        video: &mut crate::video::Video,
    ) -> bool {
        match int_num {
            0x10 => {
                self.handle_int10(memory, video);
                true
            }
            0x11 => {
                self.handle_int11(memory);
                true
            }
            0x12 => {
                self.handle_int12(memory);
                true
            }
            0x13 => {
                self.handle_int13(memory, io);
                true
            }
            0x14 => {
                self.handle_int14(memory, io);
                true
            }
            0x15 => {
                self.handle_int15(memory, io);
                true
            }
            0x16 => {
                self.handle_int16(memory, io);
                true
            }
            0x17 => {
                self.handle_int17(memory, io);
                true
            }
            0x1A => {
                self.handle_int1a(memory, io);
                true
            }
            0x21 => {
                self.handle_int21(memory, io);
                true
            }
            0x29 => {
                self.handle_int29(memory, io);
                true
            }
            0x2F => {
                self.handle_int2f(memory, io);
                true
            }
            // Other BIOS interrupts can be added here
            _ => {
                warn!("Unhandled BIOS interrupt: 0x{:02X}", int_num);
                false // Not handled, proceed with normal interrupt mechanism
            }
        }
    }
}
