// BIOS and DOS interrupt handler trait and implementation
// The core provides the interrupt dispatch mechanism, but I/O is handled via callbacks

use super::Cpu;
use crate::memory::Memory;
use log::warn;

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
}

/// A null I/O handler that does nothing (for testing or headless operation)
pub struct NullBios;

impl Bios for NullBios {
    fn read_char(&mut self) -> Option<u8> {
        None
    }

    fn write_char(&mut self, _ch: u8) {
        // Do nothing
    }

    fn write_str(&mut self, _s: &str) {
        // Do nothing
    }

    fn disk_reset(&mut self, _drive: u8) -> bool {
        false // No disk available
    }

    fn disk_read_sectors(
        &mut self,
        _drive: u8,
        _cylinder: u8,
        _head: u8,
        _sector: u8,
        _count: u8,
    ) -> Result<Vec<u8>, u8> {
        Err(disk_errors::INVALID_COMMAND)
    }

    fn disk_write_sectors(
        &mut self,
        _drive: u8,
        _cylinder: u8,
        _head: u8,
        _sector: u8,
        _count: u8,
        _data: &[u8],
    ) -> Result<u8, u8> {
        Err(disk_errors::INVALID_COMMAND)
    }

    fn disk_get_params(&self, _drive: u8) -> Result<DriveParams, u8> {
        Err(disk_errors::INVALID_COMMAND)
    }

    fn file_create(&mut self, _filename: &str, _attributes: u8) -> Result<u16, u8> {
        Err(dos_errors::ACCESS_DENIED)
    }

    fn file_open(&mut self, _filename: &str, _access_mode: u8) -> Result<u16, u8> {
        Err(dos_errors::FILE_NOT_FOUND)
    }

    fn file_close(&mut self, _handle: u16) -> Result<(), u8> {
        Err(dos_errors::INVALID_HANDLE)
    }

    fn file_read(&mut self, _handle: u16, _max_bytes: u16) -> Result<Vec<u8>, u8> {
        Err(dos_errors::INVALID_HANDLE)
    }

    fn file_write(&mut self, _handle: u16, _data: &[u8]) -> Result<u16, u8> {
        Err(dos_errors::INVALID_HANDLE)
    }

    fn file_seek(&mut self, _handle: u16, _offset: i32, _method: SeekMethod) -> Result<u32, u8> {
        Err(dos_errors::INVALID_HANDLE)
    }

    fn dir_create(&mut self, _dirname: &str) -> Result<(), u8> {
        Err(dos_errors::ACCESS_DENIED)
    }

    fn dir_remove(&mut self, _dirname: &str) -> Result<(), u8> {
        Err(dos_errors::ACCESS_DENIED)
    }

    fn dir_change(&mut self, _dirname: &str) -> Result<(), u8> {
        Err(dos_errors::PATH_NOT_FOUND)
    }

    fn dir_get_current(&self, _drive: u8) -> Result<String, u8> {
        Err(dos_errors::INVALID_DRIVE)
    }

    fn find_first(&mut self, _pattern: &str, _attributes: u8) -> Result<(usize, FindData), u8> {
        Err(dos_errors::NO_MORE_FILES)
    }

    fn find_next(&mut self, _search_id: usize) -> Result<FindData, u8> {
        Err(dos_errors::NO_MORE_FILES)
    }
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
            0x13 => {
                self.handle_int13(memory, io);
                true
            }
            0x21 => {
                self.handle_int21(memory, io);
                true
            }
            // Other BIOS interrupts can be added here
            // 0x16 => Keyboard services
            // etc.
            _ => {
                warn!("Unhandled BIOS interrupt: 0x{:02X}", int_num);
                false // Not handled, proceed with normal interrupt mechanism
            }
        }
    }

    /// INT 0x21 - DOS Services
    /// AH register contains the function number
    fn handle_int21<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let function = (self.ax >> 8) as u8; // Get AH directly

        match function {
            0x01 => self.int21_read_char_with_echo(io),
            0x02 => self.int21_write_char(io),
            0x09 => self.int21_write_string(memory, io),
            0x39 => self.int21_create_dir(memory, io),
            0x3A => self.int21_remove_dir(memory, io),
            0x3B => self.int21_change_dir(memory, io),
            0x3C => self.int21_create_file(memory, io),
            0x3D => self.int21_open_file(memory, io),
            0x3E => self.int21_close_file(io),
            0x3F => self.int21_read_file(memory, io),
            0x40 => self.int21_write_file(memory, io),
            0x42 => self.int21_seek_file(io),
            0x47 => self.int21_get_current_dir(memory, io),
            0x4C => self.int21_exit(),
            0x4E => self.int21_find_first(memory, io),
            0x4F => self.int21_find_next(memory, io),
            _ => {
                warn!("Unhandled INT 0x21 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 21h, AH=01h - Read Character from STDIN with Echo
    /// Returns: AL = character read
    fn int21_read_char_with_echo<T: Bios>(&mut self, io: &mut T) {
        if let Some(ch) = io.read_char() {
            // Echo the character
            io.write_char(ch);
            // Store in AL
            self.ax = (self.ax & 0xFF00) | (ch as u16);
        }
    }

    /// INT 21h, AH=02h - Write Character to STDOUT
    /// Input: DL = character to write
    fn int21_write_char<T: Bios>(&mut self, io: &mut T) {
        let ch = self.get_reg8(2); // DL register
        io.write_char(ch);
    }

    /// INT 21h, AH=09h - Write String to STDOUT
    /// Input: DS:DX = pointer to '$'-terminated string
    fn int21_write_string<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let mut addr = Self::physical_address(self.ds, self.dx);
        let mut output = String::new();

        loop {
            let ch = memory.read_byte(addr);
            if ch == b'$' {
                break;
            }
            output.push(ch as char);
            addr += 1;
        }

        io.write_str(&output);
    }

    /// INT 21h, AH=4Ch - Exit Program
    /// Input: AL = return code
    fn int21_exit(&mut self) {
        // Halt the CPU
        self.halted = true;
    }

    /// INT 21h, AH=3Ch - Create or Truncate File
    /// Input:
    ///   DS:DX = pointer to null-terminated filename
    ///   CX = file attributes
    /// Output:
    ///   CF clear if success: AX = file handle
    ///   CF set if error: AX = error code
    fn int21_create_file<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let filename = self.read_null_terminated_string(memory, self.ds, self.dx);
        let attributes = (self.cx & 0xFF) as u8;

        match io.file_create(&filename, attributes) {
            Ok(handle) => {
                self.ax = handle;
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 21h, AH=3Dh - Open Existing File
    /// Input:
    ///   DS:DX = pointer to null-terminated filename
    ///   AL = access mode (0=read, 1=write, 2=read/write)
    /// Output:
    ///   CF clear if success: AX = file handle
    ///   CF set if error: AX = error code
    fn int21_open_file<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let filename = self.read_null_terminated_string(memory, self.ds, self.dx);
        let access_mode = (self.ax & 0xFF) as u8;

        match io.file_open(&filename, access_mode) {
            Ok(handle) => {
                self.ax = handle;
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 21h, AH=3Eh - Close File
    /// Input:
    ///   BX = file handle
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_close_file<T: Bios>(&mut self, io: &mut T) {
        let handle = self.bx;

        match io.file_close(handle) {
            Ok(()) => {
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 21h, AH=3Fh - Read from File or Device
    /// Input:
    ///   BX = file handle
    ///   CX = number of bytes to read
    ///   DS:DX = pointer to buffer
    /// Output:
    ///   CF clear if success: AX = number of bytes read
    ///   CF set if error: AX = error code
    fn int21_read_file<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let handle = self.bx;
        let max_bytes = self.cx;

        match io.file_read(handle, max_bytes) {
            Ok(data) => {
                // Write data to DS:DX
                let buffer_addr = Self::physical_address(self.ds, self.dx);
                for (i, &byte) in data.iter().enumerate() {
                    memory.write_byte(buffer_addr + i, byte);
                }
                self.ax = data.len() as u16;
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 21h, AH=40h - Write to File or Device
    /// Input:
    ///   BX = file handle
    ///   CX = number of bytes to write
    ///   DS:DX = pointer to data
    /// Output:
    ///   CF clear if success: AX = number of bytes written
    ///   CF set if error: AX = error code
    fn int21_write_file<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let handle = self.bx;
        let num_bytes = self.cx;

        // Read data from DS:DX
        let buffer_addr = Self::physical_address(self.ds, self.dx);
        let mut data = Vec::with_capacity(num_bytes as usize);
        for i in 0..num_bytes {
            data.push(memory.read_byte(buffer_addr + i as usize));
        }

        match io.file_write(handle, &data) {
            Ok(bytes_written) => {
                self.ax = bytes_written;
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 21h, AH=42h - Seek (LSEEK)
    /// Input:
    ///   BX = file handle
    ///   AL = seek method (0=from start, 1=from current, 2=from end)
    ///   CX:DX = signed offset (32-bit)
    /// Output:
    ///   CF clear if success: DX:AX = new file position
    ///   CF set if error: AX = error code
    fn int21_seek_file<T: Bios>(&mut self, io: &mut T) {
        let handle = self.bx;
        let method_code = (self.ax & 0xFF) as u8;

        // Combine CX:DX into a 32-bit signed offset
        let offset = ((self.cx as u32) << 16) | (self.dx as u32);
        let offset_signed = offset as i32;

        let method = match method_code {
            0 => SeekMethod::FromStart,
            1 => SeekMethod::FromCurrent,
            2 => SeekMethod::FromEnd,
            _ => {
                self.ax = dos_errors::INVALID_FUNCTION as u16;
                self.set_flag(super::FLAG_CARRY, true);
                return;
            }
        };

        match io.file_seek(handle, offset_signed, method) {
            Ok(new_position) => {
                // Return new position in DX:AX
                self.dx = (new_position >> 16) as u16;
                self.ax = (new_position & 0xFFFF) as u16;
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 21h, AH=39h - Create Directory (MKDIR)
    /// Input:
    ///   DS:DX = pointer to null-terminated directory name
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_create_dir<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let dirname = self.read_null_terminated_string(memory, self.ds, self.dx);

        match io.dir_create(&dirname) {
            Ok(()) => {
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 21h, AH=3Ah - Remove Directory (RMDIR)
    /// Input:
    ///   DS:DX = pointer to null-terminated directory name
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_remove_dir<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let dirname = self.read_null_terminated_string(memory, self.ds, self.dx);

        match io.dir_remove(&dirname) {
            Ok(()) => {
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 21h, AH=3Bh - Change Current Directory (CHDIR)
    /// Input:
    ///   DS:DX = pointer to null-terminated directory name
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_change_dir<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let dirname = self.read_null_terminated_string(memory, self.ds, self.dx);

        match io.dir_change(&dirname) {
            Ok(()) => {
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 21h, AH=47h - Get Current Directory
    /// Input:
    ///   DL = drive number (0=default, 1=A, 2=B, etc.)
    ///   DS:SI = pointer to 64-byte buffer for directory path
    /// Output:
    ///   CF clear if success: buffer filled with path (without drive or leading backslash)
    ///   CF set if error: AX = error code
    fn int21_get_current_dir<T: Bios>(&mut self, memory: &mut Memory, io: &T) {
        let drive = (self.dx & 0xFF) as u8; // DL

        match io.dir_get_current(drive) {
            Ok(path) => {
                // Write path to DS:SI (null-terminated)
                let buffer_addr = Self::physical_address(self.ds, self.si);
                for (i, &byte) in path.as_bytes().iter().enumerate() {
                    if i >= 63 {
                        break; // Leave room for null terminator
                    }
                    memory.write_byte(buffer_addr + i, byte);
                }
                // Write null terminator
                let len = path.len().min(63);
                memory.write_byte(buffer_addr + len, 0);

                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 21h, AH=4Eh - Find First Matching File
    /// Input:
    ///   DS:DX = pointer to null-terminated file pattern (may include wildcards)
    ///   CX = file attributes to match
    ///   ES:BX = pointer to DTA (Disk Transfer Area, 43 bytes)
    /// Output:
    ///   CF clear if success: DTA filled with file information
    ///   CF set if error: AX = error code
    fn int21_find_first<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let pattern = self.read_null_terminated_string(memory, self.ds, self.dx);
        let attributes = (self.cx & 0xFF) as u8;

        match io.find_first(&pattern, attributes) {
            Ok((search_id, find_data)) => {
                // Write search ID to a hidden location (we'll use offset 0 of DTA for this)
                let dta_addr = Self::physical_address(self.es, self.bx);

                // DOS DTA format for find first/next:
                // Offset 0-20: Reserved for DOS (we'll store search_id here)
                // Offset 21: File attributes
                // Offset 22-23: File time
                // Offset 24-25: File date
                // Offset 26-29: File size (32-bit little-endian)
                // Offset 30-42: Filename (null-terminated, 13 bytes max)

                // Store search_id in first 8 bytes (as u64)
                for i in 0..8 {
                    memory.write_byte(dta_addr + i, ((search_id >> (i * 8)) & 0xFF) as u8);
                }

                self.write_find_data_to_dta(memory, dta_addr, &find_data);
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 21h, AH=4Fh - Find Next Matching File
    /// Input:
    ///   ES:BX = pointer to DTA (must contain data from previous find first/next)
    /// Output:
    ///   CF clear if success: DTA filled with file information
    ///   CF set if error: AX = error code
    fn int21_find_next<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let dta_addr = Self::physical_address(self.es, self.bx);

        // Read search_id from DTA
        let mut search_id: usize = 0;
        for i in 0..8 {
            search_id |= (memory.read_byte(dta_addr + i) as usize) << (i * 8);
        }

        match io.find_next(search_id) {
            Ok(find_data) => {
                self.write_find_data_to_dta(memory, dta_addr, &find_data);
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// Helper function to write FindData to DTA
    fn write_find_data_to_dta(&self, memory: &mut Memory, dta_addr: usize, find_data: &FindData) {
        // Offset 21: File attributes
        memory.write_byte(dta_addr + 21, find_data.attributes);

        // Offset 22-23: File time (little-endian)
        memory.write_byte(dta_addr + 22, (find_data.time & 0xFF) as u8);
        memory.write_byte(dta_addr + 23, (find_data.time >> 8) as u8);

        // Offset 24-25: File date (little-endian)
        memory.write_byte(dta_addr + 24, (find_data.date & 0xFF) as u8);
        memory.write_byte(dta_addr + 25, (find_data.date >> 8) as u8);

        // Offset 26-29: File size (32-bit little-endian)
        memory.write_byte(dta_addr + 26, (find_data.size & 0xFF) as u8);
        memory.write_byte(dta_addr + 27, ((find_data.size >> 8) & 0xFF) as u8);
        memory.write_byte(dta_addr + 28, ((find_data.size >> 16) & 0xFF) as u8);
        memory.write_byte(dta_addr + 29, ((find_data.size >> 24) & 0xFF) as u8);

        // Offset 30-42: Filename (null-terminated, max 13 bytes)
        let filename_bytes = find_data.filename.as_bytes();
        for (i, &byte) in filename_bytes.iter().take(12).enumerate() {
            memory.write_byte(dta_addr + 30 + i, byte);
        }
        // Null terminator
        let len = filename_bytes.len().min(12);
        memory.write_byte(dta_addr + 30 + len, 0);
    }

    /// Helper function to read a null-terminated string from memory
    fn read_null_terminated_string(&self, memory: &Memory, segment: u16, offset: u16) -> String {
        let mut addr = Self::physical_address(segment, offset);
        let mut result = String::new();

        loop {
            let ch = memory.read_byte(addr);
            if ch == 0 {
                break;
            }
            result.push(ch as char);
            addr += 1;
        }

        result
    }

    /// INT 0x13 - BIOS Disk Services
    /// AH register contains the function number
    fn handle_int13<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int13_reset_disk(io),
            0x02 => self.int13_read_sectors(memory, io),
            0x03 => self.int13_write_sectors(memory, io),
            0x08 => self.int13_get_drive_params(io),
            _ => {
                warn!("Unhandled INT 0x13 function: AH=0x{:02X}", function);
                // Set error: invalid command
                self.ax = (self.ax & 0x00FF) | ((disk_errors::INVALID_COMMAND as u16) << 8);
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 13h, AH=00h - Reset Disk System
    /// Input:
    ///   DL = drive number (0x00-0x7F for floppies, 0x80-0xFF for hard disks)
    /// Output:
    ///   AH = status (0 = success)
    ///   CF = clear if success, set if error
    fn int13_reset_disk<T: Bios>(&mut self, io: &mut T) {
        let drive = (self.dx & 0xFF) as u8; // Get DL

        let success = io.disk_reset(drive);

        if success {
            self.ax &= 0x00FF; // AH = 0 (success)
            self.set_flag(super::FLAG_CARRY, false);
        } else {
            self.ax = (self.ax & 0x00FF) | ((disk_errors::RESET_FAILED as u16) << 8);
            self.set_flag(super::FLAG_CARRY, true);
        }
    }

    /// INT 13h, AH=02h - Read Sectors into Memory
    /// Input:
    ///   AL = number of sectors to read (1-128)
    ///   CH = cylinder number (0-1023, low 8 bits)
    ///   CL = sector number (1-63, bits 0-5) + high 2 bits of cylinder (bits 6-7)
    ///   DH = head number (0-255)
    ///   DL = drive number
    ///   ES:BX = buffer address
    /// Output:
    ///   AH = status (0 = success)
    ///   AL = number of sectors read
    ///   CF = clear if success, set if error
    fn int13_read_sectors<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let count = (self.ax & 0xFF) as u8; // AL
        let cylinder_low = (self.cx >> 8) as u8; // CH
        let sector_and_cyl_high = (self.cx & 0xFF) as u8; // CL
        let head = (self.dx >> 8) as u8; // DH
        let drive = (self.dx & 0xFF) as u8; // DL

        // Extract cylinder and sector from CL
        let sector = sector_and_cyl_high & 0x3F; // Bits 0-5
        // For 8086, we only support 8-bit cylinders (compatibility mode)
        let cylinder_8bit = cylinder_low;

        match io.disk_read_sectors(drive, cylinder_8bit, head, sector, count) {
            Ok(data) => {
                // Write data to ES:BX
                let buffer_addr = Self::physical_address(self.es, self.bx);
                for (i, &byte) in data.iter().enumerate() {
                    memory.write_byte(buffer_addr + i, byte);
                }

                // Calculate actual sectors read
                let sectors_read = (data.len() / 512).min(count as usize) as u8;

                self.ax = (self.ax & 0xFF00) | (sectors_read as u16); // AL = sectors read
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.ax &= 0xFF00; // AL = 0 (no sectors read)
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 13h, AH=03h - Write Sectors from Memory
    /// Input:
    ///   AL = number of sectors to write (1-128)
    ///   CH = cylinder number (0-1023, low 8 bits)
    ///   CL = sector number (1-63, bits 0-5) + high 2 bits of cylinder (bits 6-7)
    ///   DH = head number (0-255)
    ///   DL = drive number
    ///   ES:BX = buffer address
    /// Output:
    ///   AH = status (0 = success)
    ///   AL = number of sectors written
    ///   CF = clear if success, set if error
    fn int13_write_sectors<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let count = (self.ax & 0xFF) as u8; // AL
        let cylinder_low = (self.cx >> 8) as u8; // CH
        let sector_and_cyl_high = (self.cx & 0xFF) as u8; // CL
        let head = (self.dx >> 8) as u8; // DH
        let drive = (self.dx & 0xFF) as u8; // DL

        // Extract cylinder and sector from CL
        let sector = sector_and_cyl_high & 0x3F; // Bits 0-5
        // For 8086, we only support 8-bit cylinders (compatibility mode)
        let cylinder_8bit = cylinder_low;

        // Read data from ES:BX
        let buffer_addr = Self::physical_address(self.es, self.bx);
        let data_len = count as usize * 512;
        let mut data = Vec::with_capacity(data_len);
        for i in 0..data_len {
            data.push(memory.read_byte(buffer_addr + i));
        }

        match io.disk_write_sectors(drive, cylinder_8bit, head, sector, count, &data) {
            Ok(sectors_written) => {
                self.ax = (self.ax & 0xFF00) | (sectors_written as u16); // AL = sectors written
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.ax &= 0xFF00; // AL = 0 (no sectors written)
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 13h, AH=08h - Get Drive Parameters
    /// Input:
    ///   DL = drive number
    /// Output:
    ///   AH = status (0 = success)
    ///   CF = clear if success, set if error
    ///   On success:
    ///     CH = maximum cylinder number (low 8 bits)
    ///     CL = maximum sector number (bits 0-5) + high 2 bits of max cylinder (bits 6-7)
    ///     DH = maximum head number
    ///     DL = number of drives
    fn int13_get_drive_params<T: Bios>(&mut self, io: &T) {
        let drive = (self.dx & 0xFF) as u8; // Get DL

        match io.disk_get_params(drive) {
            Ok(params) => {
                // Pack cylinder into CH and CL
                let cylinder = params.max_cylinder as u16;
                let cylinder_low = (cylinder & 0xFF) as u8;
                let cylinder_high = ((cylinder >> 8) & 0x03) as u8;

                // Pack sector and cylinder high bits into CL
                let cl = (params.max_sector & 0x3F) | (cylinder_high << 6);

                self.cx = ((cylinder_low as u16) << 8) | (cl as u16); // CH:CL
                self.dx = ((params.max_head as u16) << 8) | (params.drive_count as u16); // DH:DL
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(super::FLAG_CARRY, false);
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.set_flag(super::FLAG_CARRY, true);
            }
        }
    }

    /// INT 0x10 - Video Services
    /// AH register contains the function number
    fn handle_int10(&mut self, memory: &mut Memory, video: &mut crate::video::Video) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int10_set_video_mode(video),
            0x02 => self.int10_set_cursor_position(video),
            0x06 => self.int10_scroll_up(memory, video),
            0x07 => self.int10_scroll_down(memory, video),
            0x09 => self.int10_write_char_attr(memory, video),
            0x0E => self.int10_teletype_output(memory, video),
            0x13 => self.int10_write_string(memory, video),
            _ => {
                warn!("Unhandled INT 0x10 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 10h, AH=00h - Set Video Mode
    /// Input:
    ///   AL = video mode (0x00-0x03, 0x07 for text modes)
    /// Output: None
    fn int10_set_video_mode(&mut self, video: &mut crate::video::Video) {
        let mode = (self.ax & 0xFF) as u8; // AL

        // We only support text modes (0x00-0x03, 0x07)
        if mode <= 0x07 {
            video.set_mode(mode);
            // Reset cursor to top-left
            video.set_cursor(0, 0);
        } else {
            warn!("Unsupported video mode: 0x{:02X}", mode);
        }
    }

    /// INT 10h, AH=02h - Set Cursor Position
    /// Input:
    ///   DH = row (0-24)
    ///   DL = column (0-79)
    ///   BH = page number (0 for text mode)
    /// Output: None
    fn int10_set_cursor_position(&mut self, video: &mut crate::video::Video) {
        let row = (self.dx >> 8) as u8; // DH
        let col = (self.dx & 0xFF) as u8; // DL

        if row < 25 && col < 80 {
            video.set_cursor(row as usize, col as usize);
        }
    }

    /// INT 10h, AH=06h - Scroll Up Window
    /// Input:
    ///   AL = number of lines to scroll (0 = clear entire window)
    ///   BH = attribute for blank lines
    ///   CH = row of upper-left corner of window
    ///   CL = column of upper-left corner
    ///   DH = row of lower-right corner
    ///   DL = column of lower-right corner
    /// Output: None
    fn int10_scroll_up(&mut self, memory: &mut Memory, video: &mut crate::video::Video) {
        let lines = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx >> 8) as u8; // BH
        let top = (self.cx >> 8) as u8; // CH
        let left = (self.cx & 0xFF) as u8; // CL
        let bottom = (self.dx >> 8) as u8; // DH
        let right = (self.dx & 0xFF) as u8; // DL

        // Validate bounds
        if top > bottom || left > right || bottom >= 25 || right >= 80 {
            return;
        }

        if lines == 0 {
            // Clear entire window
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row as usize * 80 + col as usize) * 2;
                    video.write_byte(offset, b' ');
                    video.write_byte(offset + 1, attr);
                }
            }
        } else {
            // Scroll up by 'lines' rows
            for row in top..=bottom {
                for col in left..=right {
                    let dest_offset = (row as usize * 80 + col as usize) * 2;
                    let src_row = row + lines;

                    if src_row <= bottom {
                        // Copy from below
                        let src_offset = (src_row as usize * 80 + col as usize) * 2;
                        let src_addr = 0xB8000 + src_offset;
                        let ch = memory.read_byte(src_addr);
                        let at = memory.read_byte(src_addr + 1);
                        video.write_byte(dest_offset, ch);
                        video.write_byte(dest_offset + 1, at);
                    } else {
                        // Fill with blanks
                        video.write_byte(dest_offset, b' ');
                        video.write_byte(dest_offset + 1, attr);
                    }
                }
            }
        }
    }

    /// INT 10h, AH=07h - Scroll Down Window
    /// Input:
    ///   AL = number of lines to scroll (0 = clear entire window)
    ///   BH = attribute for blank lines
    ///   CH = row of upper-left corner of window
    ///   CL = column of upper-left corner
    ///   DH = row of lower-right corner
    ///   DL = column of lower-right corner
    /// Output: None
    fn int10_scroll_down(&mut self, memory: &mut Memory, video: &mut crate::video::Video) {
        let lines = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx >> 8) as u8; // BH
        let top = (self.cx >> 8) as u8; // CH
        let left = (self.cx & 0xFF) as u8; // CL
        let bottom = (self.dx >> 8) as u8; // DH
        let right = (self.dx & 0xFF) as u8; // DL

        // Validate bounds
        if top > bottom || left > right || bottom >= 25 || right >= 80 {
            return;
        }

        if lines == 0 {
            // Clear entire window
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row as usize * 80 + col as usize) * 2;
                    video.write_byte(offset, b' ');
                    video.write_byte(offset + 1, attr);
                }
            }
        } else {
            // Scroll down by 'lines' rows (process bottom to top)
            for row in (top..=bottom).rev() {
                for col in left..=right {
                    let dest_offset = (row as usize * 80 + col as usize) * 2;

                    if row >= top + lines {
                        // Copy from above
                        let src_row = row - lines;
                        let src_offset = (src_row as usize * 80 + col as usize) * 2;
                        let src_addr = 0xB8000 + src_offset;
                        let ch = memory.read_byte(src_addr);
                        let at = memory.read_byte(src_addr + 1);
                        video.write_byte(dest_offset, ch);
                        video.write_byte(dest_offset + 1, at);
                    } else {
                        // Fill with blanks
                        video.write_byte(dest_offset, b' ');
                        video.write_byte(dest_offset + 1, attr);
                    }
                }
            }
        }
    }

    /// INT 10h, AH=09h - Write Character and Attribute at Cursor
    /// Input:
    ///   AL = character to write
    ///   BL = attribute byte (foreground/background color)
    ///   BH = page number (0 for text mode)
    ///   CX = number of times to write character
    /// Output: None (cursor position unchanged)
    fn int10_write_char_attr(&mut self, _memory: &mut Memory, video: &mut crate::video::Video) {
        let ch = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx & 0xFF) as u8; // BL
        let count = self.cx;
        let cursor = video.get_cursor();

        for i in 0..count {
            let pos = cursor.row * 80 + cursor.col + (i as usize);
            if pos >= 80 * 25 {
                break; // Don't write beyond screen
            }
            let offset = pos * 2;
            video.write_byte(offset, ch);
            video.write_byte(offset + 1, attr);
        }
        // Cursor position is NOT updated by this function
    }

    /// INT 10h, AH=0Eh - Teletype Output
    /// Input:
    ///   AL = character to write
    ///   BL = foreground color (in graphics modes)
    ///   BH = page number (0 for text mode)
    /// Output: None
    fn int10_teletype_output(&mut self, memory: &mut Memory, video: &mut crate::video::Video) {
        let ch = (self.ax & 0xFF) as u8; // AL
        let cursor = video.get_cursor();

        match ch {
            b'\r' => {
                // Carriage return - move to column 0
                video.set_cursor(cursor.row, 0);
            }
            b'\n' => {
                // Line feed - move to next line
                let new_row = if cursor.row >= 24 {
                    // Need to scroll
                    self.scroll_up_internal(memory, video, 1);
                    24
                } else {
                    cursor.row + 1
                };
                video.set_cursor(new_row, cursor.col);
            }
            b'\x08' => {
                // Backspace
                if cursor.col > 0 {
                    video.set_cursor(cursor.row, cursor.col - 1);
                }
            }
            _ => {
                // Normal character - write and advance
                let offset = (cursor.row * 80 + cursor.col) * 2;
                video.write_byte(offset, ch);
                // Don't modify attribute byte (preserve existing color)

                // Advance cursor
                let new_col = cursor.col + 1;
                if new_col >= 80 {
                    // Wrap to next line
                    let new_row = if cursor.row >= 24 {
                        self.scroll_up_internal(memory, video, 1);
                        24
                    } else {
                        cursor.row + 1
                    };
                    video.set_cursor(new_row, 0);
                } else {
                    video.set_cursor(cursor.row, new_col);
                }
            }
        }
    }

    /// INT 10h, AH=13h - Write String
    /// Input:
    ///   AL = write mode (bit 0: update cursor, bit 1: string has attributes)
    ///   BH = page number
    ///   BL = attribute (if mode bit 1 = 0)
    ///   CX = string length
    ///   DH = row
    ///   DL = column
    ///   ES:BP = pointer to string
    /// Output: None
    fn int10_write_string(&mut self, memory: &Memory, video: &mut crate::video::Video) {
        let mode = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx & 0xFF) as u8; // BL
        let length = self.cx;
        let row = (self.dx >> 8) as u8; // DH
        let col = (self.dx & 0xFF) as u8; // DL

        let update_cursor = (mode & 0x01) != 0;
        let has_attributes = (mode & 0x02) != 0;

        // Set initial position
        video.set_cursor(row as usize, col as usize);

        let mut addr = Self::physical_address(self.es, self.bp);

        for _ in 0..length {
            let ch = memory.read_byte(addr);
            addr += 1;

            let current_attr = if has_attributes {
                let a = memory.read_byte(addr);
                addr += 1;
                a
            } else {
                attr
            };

            let cursor = video.get_cursor();
            if cursor.row >= 25 {
                break;
            }

            let offset = (cursor.row * 80 + cursor.col) * 2;
            video.write_byte(offset, ch);
            video.write_byte(offset + 1, current_attr);

            // Advance cursor (even if not updating final position)
            let new_col = cursor.col + 1;
            if new_col >= 80 {
                video.set_cursor(cursor.row + 1, 0);
            } else {
                video.set_cursor(cursor.row, new_col);
            }
        }

        // Restore cursor if mode doesn't update it
        if !update_cursor {
            video.set_cursor(row as usize, col as usize);
        }
    }

    /// Helper function for internal scrolling (used by teletype)
    fn scroll_up_internal(&mut self, memory: &mut Memory, video: &mut crate::video::Video, lines: u8) {
        // Save registers
        let saved_ax = self.ax;
        let saved_bx = self.bx;
        let saved_cx = self.cx;
        let saved_dx = self.dx;

        // Set up parameters for scroll_up
        self.ax = (self.ax & 0xFF00) | (lines as u16); // AL = lines
        self.bx = 0x0700; // BH = 0x07 (white on black)
        self.cx = 0x0000; // CH=0, CL=0 (top-left)
        self.dx = 0x184F; // DH=24, DL=79 (bottom-right)

        self.int10_scroll_up(memory, video);

        // Restore registers
        self.ax = saved_ax;
        self.bx = saved_bx;
        self.cx = saved_cx;
        self.dx = saved_dx;
    }
}
