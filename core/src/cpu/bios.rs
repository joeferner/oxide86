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
            0x4C => self.int21_exit(),
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
