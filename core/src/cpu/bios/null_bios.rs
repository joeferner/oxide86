use crate::{
    Bios, DriveParams,
    cpu::bios::{
        ExecParams, FindData, KeyPress, PrinterStatus, RtcDate, RtcTime, SeekMethod, SerialParams,
        SerialStatus, dos_errors, int14::line_status, int17::printer_status,
    },
    disk_errors,
};

/// A null I/O handler that does nothing (for testing or headless operation)
pub struct NullBios;

impl Bios for NullBios {
    fn get_current_drive(&self) -> u8 {
        0 // Default to drive A
    }

    fn set_default_drive(&mut self, _drive: u8) -> u8 {
        1 // Return 1 logical drive (A:)
    }

    fn memory_allocate(&mut self, _paragraphs: u16) -> Result<u16, (u8, u16)> {
        Err((dos_errors::INSUFFICIENT_MEMORY, 0))
    }

    fn memory_free(&mut self, _segment: u16) -> Result<(), u8> {
        Err(dos_errors::INVALID_MEMORY_BLOCK_ADDRESS)
    }

    fn memory_resize(&mut self, _segment: u16, _paragraphs: u16) -> Result<(), (u8, u16)> {
        Err((dos_errors::INVALID_MEMORY_BLOCK_ADDRESS, 0))
    }

    fn get_psp(&self) -> u16 {
        0x0000 // Default PSP segment
    }

    fn set_psp(&mut self, _segment: u16) {
        // Do nothing in null implementation
    }

    fn ioctl_get_device_info(&self, handle: u16) -> Result<u16, u8> {
        // Return device information for standard handles
        match handle {
            0 => Ok(0x80D0), // STDIN: device, no EOF, binary mode
            1 => Ok(0x80D1), // STDOUT: device, no EOF, binary mode
            2 => Ok(0x80D1), // STDERR: device, no EOF, binary mode
            _ => Err(dos_errors::INVALID_HANDLE),
        }
    }

    fn ioctl_set_device_info(&mut self, handle: u16, _info: u16) -> Result<(), u8> {
        // Only allow setting info for standard handles
        match handle {
            0..=2 => Ok(()),
            _ => Err(dos_errors::INVALID_HANDLE),
        }
    }

    fn exec_load_program(&mut self, _params: &ExecParams) -> Result<Vec<u8>, u8> {
        // No file system available in null implementation
        Err(dos_errors::FILE_NOT_FOUND)
    }

    fn read_char(&mut self) -> Option<u8> {
        None
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

    fn disk_get_type(&self, _drive: u8) -> Result<(u8, u32), u8> {
        Err(disk_errors::INVALID_COMMAND)
    }

    fn disk_detect_change(&mut self, _drive: u8) -> Result<bool, u8> {
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

    fn file_duplicate(&mut self, _handle: u16) -> Result<u16, u8> {
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
