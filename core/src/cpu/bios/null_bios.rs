use crate::{Bios, DriveParams, cpu::bios::{FindData, KeyPress, SeekMethod, SerialParams, SerialStatus, dos_errors, int14::line_status}, disk_errors};

/// A null I/O handler that does nothing (for testing or headless operation)
pub struct NullBios;

impl Bios for NullBios {
    fn get_current_drive(&self) -> u8 {
        0 // Default to drive A
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

    fn get_system_ticks(&self) -> u32 {
        0 // No real time available in null implementation
    }
}
