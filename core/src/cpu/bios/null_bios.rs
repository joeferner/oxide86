use crate::{
    Bios, DriveNumber, DriveParams,
    cpu::bios::{
        ExecParams, FileAccess, FindData, KeyPress, PrinterStatus, RtcDate, RtcTime, SeekMethod,
        SerialParams, SerialStatus, disk_error::DiskError, dos_error::DosError, int14::line_status,
        int17::printer_status,
    },
};

/// A null I/O handler that does nothing (for testing or headless operation)
pub struct NullBios;

impl Bios for NullBios {
    fn get_current_drive(&self) -> DriveNumber {
        DriveNumber::floppy_a()
    }

    fn set_default_drive(&mut self, _drive: DriveNumber) -> u8 {
        1 // Return 1 logical drive (A:)
    }

    fn memory_allocate(&mut self, _paragraphs: u16) -> Result<u16, (DosError, u16)> {
        Err((DosError::InsufficientMemory, 0))
    }

    fn memory_free(&mut self, _segment: u16) -> Result<(), DosError> {
        Err(DosError::InvalidMemoryBlockAddress)
    }

    fn memory_resize(&mut self, _segment: u16, _paragraphs: u16) -> Result<(), (DosError, u16)> {
        Err((DosError::InvalidMemoryBlockAddress, 0))
    }

    fn get_psp(&self) -> u16 {
        0x0000 // Default PSP segment
    }

    fn set_psp(&mut self, _segment: u16) {
        // Do nothing in null implementation
    }

    fn ioctl_get_device_info(&self, handle: u16) -> Result<u16, DosError> {
        // Return device information for standard handles
        match handle {
            0 => Ok(0x80D0), // STDIN: device, no EOF, binary mode
            1 => Ok(0x80D1), // STDOUT: device, no EOF, binary mode
            2 => Ok(0x80D1), // STDERR: device, no EOF, binary mode
            _ => Err(DosError::InvalidHandle),
        }
    }

    fn ioctl_set_device_info(&mut self, handle: u16, _info: u16) -> Result<(), DosError> {
        // Only allow setting info for standard handles
        match handle {
            0..=2 => Ok(()),
            _ => Err(DosError::InvalidHandle),
        }
    }

    fn exec_load_program(&mut self, _params: &ExecParams) -> Result<Vec<u8>, DosError> {
        // No file system available in null implementation
        Err(DosError::FileNotFound)
    }

    fn read_char(&mut self) -> Option<u8> {
        None
    }

    fn check_char(&mut self) -> Option<u8> {
        None
    }

    fn has_char_available(&self) -> bool {
        false
    }

    fn write_char(&mut self, _ch: u8) {
        // Do nothing
    }

    fn write_str(&mut self, _s: &str) {
        // Do nothing
    }

    fn read_key(&mut self) -> Option<KeyPress> {
        None
    }

    fn check_key(&mut self) -> Option<KeyPress> {
        None
    }

    fn disk_reset(&mut self, _drive: DriveNumber) -> bool {
        false // No disk available
    }

    fn disk_read_sectors(
        &mut self,
        _drive: DriveNumber,
        _cylinder: u8,
        _head: u8,
        _sector: u8,
        _count: u8,
    ) -> Result<Vec<u8>, DiskError> {
        Err(DiskError::InvalidCommand)
    }

    fn disk_write_sectors(
        &mut self,
        _drive: DriveNumber,
        _cylinder: u8,
        _head: u8,
        _sector: u8,
        _count: u8,
        _data: &[u8],
    ) -> Result<u8, DiskError> {
        Err(DiskError::InvalidCommand)
    }

    fn disk_get_params(&self, _drive: DriveNumber) -> Result<DriveParams, DiskError> {
        Err(DiskError::InvalidCommand)
    }

    fn disk_get_type(&self, _drive: DriveNumber) -> Result<(u8, u32), DiskError> {
        Err(DiskError::InvalidCommand)
    }

    fn disk_detect_change(&mut self, _drive: DriveNumber) -> Result<bool, DiskError> {
        Err(DiskError::InvalidCommand)
    }

    fn disk_format_track(
        &mut self,
        _drive: DriveNumber,
        _cylinder: u8,
        _head: u8,
        _sectors_per_track: u8,
    ) -> Result<(), DiskError> {
        Err(DiskError::InvalidCommand)
    }

    fn disk_read_sectors_lba(
        &mut self,
        _drive: DriveNumber,
        _start_sector: u32,
        _count: u16,
    ) -> Result<Vec<u8>, DiskError> {
        Err(DiskError::InvalidCommand)
    }

    fn file_create(&mut self, _filename: &str, _attributes: u8) -> Result<u16, DosError> {
        Err(DosError::AccessDenied)
    }

    fn file_open(&mut self, _filename: &str, _access_mode: FileAccess) -> Result<u16, DosError> {
        Err(DosError::FileNotFound)
    }

    fn file_close(&mut self, _handle: u16) -> Result<(), DosError> {
        Err(DosError::InvalidHandle)
    }

    fn file_read(&mut self, _handle: u16, _max_bytes: u16) -> Result<Vec<u8>, DosError> {
        Err(DosError::InvalidHandle)
    }

    fn file_write(&mut self, _handle: u16, _data: &[u8]) -> Result<u16, DosError> {
        Err(DosError::InvalidHandle)
    }

    fn file_seek(
        &mut self,
        _handle: u16,
        _offset: i32,
        _method: SeekMethod,
    ) -> Result<u32, DosError> {
        Err(DosError::InvalidHandle)
    }

    fn file_duplicate(&mut self, _handle: u16) -> Result<u16, DosError> {
        Err(DosError::InvalidHandle)
    }

    fn dir_create(&mut self, _dirname: &str) -> Result<(), DosError> {
        Err(DosError::AccessDenied)
    }

    fn dir_remove(&mut self, _dirname: &str) -> Result<(), DosError> {
        Err(DosError::AccessDenied)
    }

    fn dir_change(&mut self, _dirname: &str) -> Result<(), DosError> {
        Err(DosError::PathNotFound)
    }

    fn dir_get_current(&self, _drive: DriveNumber) -> Result<String, DosError> {
        Err(DosError::InvalidDrive)
    }

    fn find_first(
        &mut self,
        _pattern: &str,
        _attributes: u8,
    ) -> Result<(usize, FindData), DosError> {
        Err(DosError::NoMoreFiles)
    }

    fn find_next(&mut self, _search_id: usize) -> Result<FindData, DosError> {
        Err(DosError::NoMoreFiles)
    }

    fn serial_init(&mut self, _port: u8, _params: SerialParams) -> SerialStatus {
        // No serial port available - return timeout status
        SerialStatus {
            line_status: line_status::TIMEOUT,
            modem_status: 0,
        }
    }

    fn serial_write(&mut self, _port: u8, _ch: u8) -> u8 {
        // No serial port available - return timeout
        line_status::TIMEOUT
    }

    fn serial_read(&mut self, _port: u8) -> Result<(u8, u8), u8> {
        // No serial port available - return timeout error
        Err(line_status::TIMEOUT)
    }

    fn serial_status(&self, _port: u8) -> SerialStatus {
        // No serial port available - return timeout status
        SerialStatus {
            line_status: line_status::TIMEOUT,
            modem_status: 0,
        }
    }

    fn printer_init(&mut self, _printer: u8) -> PrinterStatus {
        // No printer available - return timeout status
        PrinterStatus {
            status: printer_status::TIMEOUT,
        }
    }

    fn printer_write(&mut self, _printer: u8, _ch: u8) -> PrinterStatus {
        // No printer available - return timeout status
        PrinterStatus {
            status: printer_status::TIMEOUT,
        }
    }

    fn printer_status(&self, _printer: u8) -> PrinterStatus {
        // No printer available - return timeout status
        PrinterStatus {
            status: printer_status::TIMEOUT,
        }
    }

    fn get_system_ticks(&self) -> u32 {
        0 // No real time available in null implementation
    }

    fn get_rtc_time(&self) -> Option<RtcTime> {
        None // No RTC available in null implementation
    }

    fn get_rtc_date(&self) -> Option<RtcDate> {
        None // No RTC available in null implementation
    }
}
